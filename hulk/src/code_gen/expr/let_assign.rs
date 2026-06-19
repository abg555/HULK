use crate::ast::{AssignExpr, IndexExpr, KindExpr, LetExpr, MemberAccessExpr};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;
use inkwell::AddressSpace;
use inkwell::types::BasicType;

use super::super::{CodegenValue, CodeGenerator, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_let(
        &mut self,
        let_expr: &LetExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        self.enter_scope();

        for binding in &let_expr.bindings {
            let value = self.lower_expr(&binding.initializer, analysis)?;
            let kind = value.kind();
            let ptr = self.alloca_for_kind(&kind, &binding.name)?;
            self.store_value(ptr, value)?;

            self.insert_var(
                binding.name.clone(),
                VarInfo {
                    ptr,
                    kind,
                },
            );
        }

        let result = self.lower_expr(&let_expr.body, analysis);
        self.exit_scope();
        result
    }

    pub(super) fn lower_assign(
        &mut self,
        assign: &AssignExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        match &assign.target.kind {
            KindExpr::Variable(variable) => {
                let info = self
                    .lookup_var(&variable.name)
                    .ok_or_else(|| format!("Variable no definida: {}", variable.name))?;
                let value = self.lower_expr(&assign.value, analysis)?;

                if value.kind() != info.kind {
                    return Err(format!(
                        "Tipo incompatible en asignacion a {}",
                        variable.name
                    ));
                }

                self.store_value(info.ptr, value)?;
                self.load_value(&info.kind, info.ptr, &format!("reload_{}", variable.name))
            }
            KindExpr::MemberAccess(member) => self.lower_member_assign(member, assign, analysis),
            KindExpr::Index(index_expr) => self.lower_index_assign(index_expr, assign, analysis),
            _ => Err("Asignacion solo soporta variables y miembros".to_string()),
        }
    }

    fn lower_member_assign(
        &mut self,
        member: &MemberAccessExpr,
        assign: &AssignExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let object_type = self.member_object_type(member, analysis)?;
        let object_struct = self.object_struct_type(&object_type)?;
        let object_value = self.lower_expr(&member.object, analysis)?.into_object()?;
        let typed_ptr = self
            .builder
            .build_pointer_cast(
                object_value,
                object_struct.ptr_type(AddressSpace::default()),
                "obj_cast",
            )
            .map_err(|e| e.to_string())?;

        let field_type =
            self.object_field_semantic_type(&object_type, &member.field, analysis)?;
        let field_kind = self.value_kind_from_semantic(&field_type)?;

        let value = self.lower_expr(&assign.value, analysis)?;
        if value.kind() != field_kind {
            return Err(format!(
                "Tipo incompatible en asignacion a {}",
                member.field
            ));
        }

        let field_index = self
            .object_field_names(&object_type)?
            .iter()
            .position(|name| name == &member.field)
            .ok_or_else(|| {
                format!(
                    "El tipo {} no define el campo {}",
                    object_type, member.field
                )
            })?
            + 1;

        let field_ptr = self
            .builder
            .build_struct_gep(
                object_struct,
                typed_ptr,
                field_index as u32,
                &format!("{}_field", member.field),
            )
            .map_err(|e| e.to_string())?;

        self.store_value(field_ptr, value)?;
        self.load_value(&field_kind, field_ptr, &format!("reload_{}", member.field))
    }

    fn lower_index_assign(
        &mut self,
        index_expr: &IndexExpr,
        assign: &AssignExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let vec_val = self.lower_expr(&index_expr.object, analysis)?;
        let vec_ptr = match vec_val {
            CodegenValue::Vector(ptr) => ptr,
            _ => return Err("Asignacion indexada solo soportada en vectores".to_string()),
        };

        let Some(SemanticType::Vector(inner)) = analysis.inferred_types.get(&index_expr.object.id) else {
            return Err("No se encontro el tipo inferido del vector en asignacion indexada".to_string());
        };
        let elem_sem = *inner.clone();
        let elem_kind = self.value_kind_from_semantic(&elem_sem)?;
        let elem_basic = self.basic_type_for_semantic(&elem_sem)?;

        let idx_val = self.lower_expr(&index_expr.index, analysis)?;
        let idx_i64 = match idx_val {
            CodegenValue::Number(n) => self
                .builder
                .build_float_to_signed_int(n, self.context.i64_type(), "idx_i64")
                .map_err(|e| e.to_string())?,
            _ => return Err("Indice debe ser Number".to_string()),
        };

        let elem_size = elem_basic
            .size_of()
            .ok_or_else(|| "No se pudo obtener tamano de elemento".to_string())?;
        let elem_size_i64 = self
            .builder
            .build_int_cast(elem_size, self.context.i64_type(), "elem_size")
            .map_err(|e| e.to_string())?;

        let data_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_ptr, 1, "vec_data_slot")
            .map_err(|e| e.to_string())?;
        let data_i8 = self
            .builder
            .build_load(
                self.context.i8_type().ptr_type(AddressSpace::default()),
                data_slot,
                "data_ptr",
            )
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let byte_offset = self
            .builder
            .build_int_mul(idx_i64, elem_size_i64, "byte_offset")
            .map_err(|e| e.to_string())?;

        let elem_i8_ptr = unsafe {
            self.builder
                .build_in_bounds_gep(self.context.i8_type(), data_i8, &[byte_offset], "elem_i8")
                .map_err(|e| e.to_string())?
        };

        let elem_ptr_type = match elem_basic {
            inkwell::types::BasicTypeEnum::FloatType(ft) => ft.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::IntType(it) => it.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::PointerType(pt) => pt.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::StructType(st) => st.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::ArrayType(at) => at.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::VectorType(vt) => vt.ptr_type(AddressSpace::default()),
        };

        let elem_ptr = self
            .builder
            .build_pointer_cast(elem_i8_ptr, elem_ptr_type, "elem_ptr")
            .map_err(|e| e.to_string())?;

        let value = self.lower_expr(&assign.value, analysis)?;
        self.store_value(elem_ptr, value.clone())?;
        self.load_value(&elem_kind, elem_ptr, "reload_idx_elem")
    }
}
