use inkwell::AddressSpace;
use inkwell::values::PointerValue;

use crate::ast::{MemberAccessExpr, NewExpr};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::super::{CodeGenerator, CodegenValue, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_new(
        &mut self,
        new_expr: &NewExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let decl = self
            .type_decls
            .get(&new_expr.type_name)
            .cloned()
            .ok_or_else(|| format!("Tipo no definido: {}", new_expr.type_name))?;

        if decl.param.len() != new_expr.arguments.len() {
            return Err(format!(
                "Aridad invalida al construir {}: se esperaban {} argumentos y llegaron {}",
                new_expr.type_name,
                decl.param.len(),
                new_expr.arguments.len()
            ));
        }

        let object_struct = self.object_struct_type(&new_expr.type_name)?;
        let size_value = object_struct
            .size_of()
            .ok_or_else(|| format!("No se pudo calcular el tamano de {}", new_expr.type_name))?;
        let malloc_fn = self.get_malloc_function();
        let size_value = self
            .builder
            .build_int_cast(size_value, self.context.i64_type(), "obj_size")
            .map_err(|e| e.to_string())?;
        let raw_ptr = self
            .builder
            .build_call(malloc_fn, &[size_value.into()], "obj_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();

        let typed_ptr = self
            .builder
            .build_pointer_cast(
                raw_ptr,
                object_struct.ptr_type(AddressSpace::default()),
                "obj_typed",
            )
            .map_err(|e| e.to_string())?;

        self.enter_scope();

        let self_slot = self.alloca_for_kind(&ValueKind::Object, "self")?;
        self.store_value(self_slot, CodegenValue::Object(raw_ptr))?;
        self.insert_var(
            "self".to_string(),
            VarInfo {
                ptr: self_slot,
                kind: ValueKind::Object,
            },
        );

        self.initialize_object_fields(
            &new_expr.type_name,
            &new_expr.type_name,
            &new_expr.arguments,
            analysis,
            object_struct,
            typed_ptr,
        )?;

        self.exit_scope();

        Ok(CodegenValue::Object(raw_ptr))
    }

    pub(super) fn lower_member_access(
        &mut self,
        member: &MemberAccessExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let object_type = self.member_object_type(member, analysis)?;
        let object_struct = self.object_struct_type(&object_type)?;
        let object_value = self.lower_expr(&member.object, analysis)?.into_object()?;
        let typed_ptr = self.cast_object_ptr(&object_value, &object_type)?;

        let field_type = self.object_field_semantic_type(&object_type, &member.field, analysis)?;
        let field_kind = self.value_kind_from_semantic(&field_type)?;
        let field_index = self.field_index(&object_type, &member.field)?;
        let field_ptr = self
            .builder
            .build_struct_gep(
                object_struct,
                typed_ptr,
                field_index as u32,
                &format!("{}_field", member.field),
            )
            .map_err(|e| e.to_string())?;

        self.load_value(&field_kind, field_ptr, &format!("load_{}", member.field))
    }

    fn initialize_object_fields(
        &mut self,
        concrete_type: &str,
        type_name: &str,
        arg_exprs: &[crate::ast::Expr],
        analysis: &SemanticAnalysis,
        object_struct: inkwell::types::StructType<'ctx>,
        typed_ptr: PointerValue<'ctx>,
    ) -> Result<(), String> {
        let decl = self
            .type_decls
            .get(type_name)
            .cloned()
            .ok_or_else(|| format!("Tipo no definido: {}", type_name))?;

        if decl.param.len() != arg_exprs.len() {
            return Err(format!(
                "Aridad invalida al construir {}: se esperaban {} argumentos y llegaron {}",
                type_name,
                decl.param.len(),
                arg_exprs.len()
            ));
        }

        for (param, arg_expr) in decl.param.iter().zip(arg_exprs.iter()) {
            let value = self.lower_expr(arg_expr, analysis)?;
            let slot = self.alloca_for_kind(&value.kind(), &param.name)?;
            self.store_value(slot, value)?;
            self.insert_var(
                param.name.clone(),
                VarInfo {
                    ptr: slot,
                    kind: value.kind(),
                },
            );
        }

        if let Some(parent) = &decl.parent {
            let SemanticType::Custom(parent_name) = SemanticType::from_type_ref(parent) else {
                return Err(format!("El padre de {} debe ser un tipo nombrado", type_name));
            };

            self.initialize_object_fields(
                concrete_type,
                &parent_name,
                &decl.parent_arg,
                analysis,
                object_struct,
                typed_ptr,
            )?;
        }

        for field in &decl.fields {
            let initializer = self.lower_expr(&field.initializer, analysis)?;
            let field_index = self.field_index(concrete_type, &field.name)?;
            let field_ptr = self
                .builder
                .build_struct_gep(
                    object_struct,
                    typed_ptr,
                    field_index as u32,
                    &format!("{}_field", field.name),
                )
                .map_err(|e| e.to_string())?;
            self.store_value(field_ptr, initializer)?;
        }

        Ok(())
    }

    pub(super) fn member_object_type(
        &self,
        member: &MemberAccessExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<String, String> {
        match analysis.inferred_types.get(&member.object.id) {
            Some(SemanticType::Custom(name)) => Ok(name.clone()),
            Some(other) => Err(format!(
                "Acceso a miembro sobre un valor no objeto: {}",
                other
            )),
            None => Err("No se encontro el tipo inferido del objeto".to_string()),
        }
    }

    fn field_index(&self, type_name: &str, field_name: &str) -> Result<usize, String> {
        self.object_field_names(type_name)?
            .iter()
            .position(|name| name == field_name)
            .ok_or_else(|| format!("El tipo {} no define el campo {}", type_name, field_name))
    }

    fn cast_object_ptr(
        &self,
        object_value: &PointerValue<'ctx>,
        type_name: &str,
    ) -> Result<PointerValue<'ctx>, String> {
        let object_struct = self.object_struct_type(type_name)?;
        self.builder
            .build_pointer_cast(
                *object_value,
                object_struct.ptr_type(AddressSpace::default()),
                "obj_cast",
            )
            .map_err(|e| e.to_string())
    }

    fn get_malloc_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("malloc") {
            return function;
        }

        let i8_ptr_type = self.context.i8_type().ptr_type(AddressSpace::default());
        let fn_type = i8_ptr_type.fn_type(&[self.context.i64_type().into()], false);
        self.module.add_function("malloc", fn_type, None)
    }
}
