use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::values::PointerValue;

use crate::ast::{AsExpr, IsExpr, MemberAccessExpr, NewExpr};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::super::{CodeGenerator, CodegenValue, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_is(
        &mut self,
        is_expr: &IsExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let value = self.lower_expr(&is_expr.expression, analysis)?;
        let target = SemanticType::from_type_ref(&is_expr.type_info);

        match value {
            CodegenValue::Number(_) => {
                let result = matches!(target, SemanticType::Number);
                Ok(CodegenValue::Bool(
                    self.bool_type.const_int(u64::from(result), false),
                ))
            }
            CodegenValue::Bool(_) => {
                let result = matches!(target, SemanticType::Boolean);
                Ok(CodegenValue::Bool(
                    self.bool_type.const_int(u64::from(result), false),
                ))
            }
            CodegenValue::String(_) => {
                let result = matches!(target, SemanticType::String);
                Ok(CodegenValue::Bool(
                    self.bool_type.const_int(u64::from(result), false),
                ))
            }
            CodegenValue::Object(object_value) => {
                let SemanticType::Custom(target_name) = target else {
                    return Ok(CodegenValue::Bool(self.bool_type.const_int(0, false)));
                };

                let Some(SemanticType::Custom(static_name)) =
                    analysis.inferred_types.get(&is_expr.expression.id)
                else {
                    return Ok(CodegenValue::Bool(self.bool_type.const_int(0, false)));
                };

                let type_id = self.load_type_id(object_value, static_name)?;
                let ids = self.subtype_ids(&target_name, analysis)?;

                let mut current = None;
                for id in ids {
                    let expected = self.context.i64_type().const_int(id, false);
                    let cmp = self
                        .builder
                        .build_int_compare(IntPredicate::EQ, type_id, expected, "is_cmp")
                        .map_err(|e| e.to_string())?;
                    current = Some(match current {
                        Some(accum) => self
                            .builder
                            .build_or(accum, cmp, "is_or")
                            .map_err(|e| e.to_string())?,
                        None => cmp,
                    });
                }

                let result = current.unwrap_or_else(|| self.bool_type.const_int(0, false));
                Ok(CodegenValue::Bool(result))
            }
            CodegenValue::Vector(_) => Ok(CodegenValue::Bool(self.bool_type.const_int(0, false))),
        }
    }

    pub(super) fn lower_as(
        &mut self,
        as_expr: &AsExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let value = self.lower_expr(&as_expr.expression, analysis)?;
        let target = SemanticType::from_type_ref(&as_expr.type_info);

        match value {
            CodegenValue::Number(num) => {
                if matches!(target, SemanticType::Number) {
                    Ok(CodegenValue::Number(num))
                } else {
                    Err("Cast 'as' no soportado para Number".to_string())
                }
            }
            CodegenValue::Bool(boolean) => {
                if matches!(target, SemanticType::Boolean) {
                    Ok(CodegenValue::Bool(boolean))
                } else {
                    Err("Cast 'as' no soportado para Boolean".to_string())
                }
            }
            CodegenValue::String(string) => {
                if matches!(target, SemanticType::String) {
                    Ok(CodegenValue::String(string))
                } else {
                    Err("Cast 'as' no soportado para String".to_string())
                }
            }
            CodegenValue::Object(object_value) => {
                let SemanticType::Custom(target_name) = target else {
                    return Err("Cast 'as' no soportado para tipo no objeto".to_string());
                };

                let Some(SemanticType::Custom(static_name)) =
                    analysis.inferred_types.get(&as_expr.expression.id)
                else {
                    return Err("No se encontro el tipo inferido para el cast 'as'".to_string());
                };

                let type_id = self.load_type_id(object_value, static_name)?;
                let ids = self.subtype_ids(&target_name, analysis)?;
                let mut current = None;
                for id in ids {
                    let expected = self.context.i64_type().const_int(id, false);
                    let cmp = self
                        .builder
                        .build_int_compare(IntPredicate::EQ, type_id, expected, "as_cmp")
                        .map_err(|e| e.to_string())?;
                    current = Some(match current {
                        Some(accum) => self
                            .builder
                            .build_or(accum, cmp, "as_or")
                            .map_err(|e| e.to_string())?,
                        None => cmp,
                    });
                }

                let matches = current.unwrap_or_else(|| self.bool_type.const_int(0, false));
                let function = self
                    .builder
                    .get_insert_block()
                    .and_then(|block| block.get_parent())
                    .ok_or_else(|| {
                        "No se pudo determinar la funcion actual para 'as'".to_string()
                    })?;
                let ok_block = self.context.append_basic_block(function, "as_ok");
                let fail_block = self.context.append_basic_block(function, "as_fail");
                let cont_block = self.context.append_basic_block(function, "as_cont");

                self.builder
                    .build_conditional_branch(matches, ok_block, fail_block)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(fail_block);
                let panic_fn = self.get_panic_function();
                let msg_name = self.fresh_tmp("as_panic_msg");
                let msg = self
                    .builder
                    .build_global_string_ptr("Runtime error: cast 'as' failed", &msg_name)
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_call(panic_fn, &[msg.as_pointer_value().into()], "as_panic")
                    .map_err(|e| e.to_string())?;
                self.builder
                    .build_unreachable()
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(ok_block);
                self.builder
                    .build_unconditional_branch(cont_block)
                    .map_err(|e| e.to_string())?;

                self.builder.position_at_end(cont_block);
                Ok(CodegenValue::Object(object_value))
            }
            CodegenValue::Vector(vector) => {
                if matches!(target, SemanticType::Vector(_)) {
                    Ok(CodegenValue::Vector(vector))
                } else {
                    Err("Cast 'as' no soportado para Vector".to_string())
                }
            }
        }
    }

    pub(super) fn get_panic_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("hulk_panic") {
            return function;
        }

        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let fn_type = self.context.void_type().fn_type(&[i8_ptr.into()], false);
        self.module.add_function("hulk_panic", fn_type, None)
    }

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

        let expected_ctor_len = analysis
            .type_shapes
            .get(&new_expr.type_name)
            .map(|s| s.ctor_params.len())
            .unwrap_or(decl.param.len());

        if expected_ctor_len != new_expr.arguments.len() {
            return Err(format!(
                "Aridad invalida al construir {}: se esperaban {} argumentos y llegaron {}",
                new_expr.type_name,
                expected_ctor_len,
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

        let vtable_ptr = self.vtable_global(&new_expr.type_name)?.as_pointer_value();
        let vtable_slot = self
            .builder
            .build_struct_gep(object_struct, typed_ptr, 0, "vtable_slot")
            .map_err(|e| e.to_string())?;
        let vtable_cast = self
            .builder
            .build_pointer_cast(
                vtable_ptr,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "vtable_cast",
            )
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(vtable_slot, vtable_cast)
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

        let expected_ctor_len = analysis
            .type_shapes
            .get(type_name)
            .map(|s| s.ctor_params.len())
            .unwrap_or(decl.param.len());

        if expected_ctor_len != arg_exprs.len() {
            return Err(format!(
                "Aridad invalida al construir {}: se esperaban {} argumentos y llegaron {}",
                type_name,
                expected_ctor_len,
                arg_exprs.len()
            ));
        }

        let ctor_param_names: Vec<String> = if decl.param.is_empty() {
            analysis
                .type_shapes
                .get(type_name)
                .map(|shape| shape.ctor_param_names.clone())
                .unwrap_or_else(|| decl.param.iter().map(|param| param.name.clone()).collect())
        } else {
            decl.param.iter().map(|param| param.name.clone()).collect()
        };

        for (param_name, arg_expr) in ctor_param_names.iter().zip(arg_exprs.iter()) {
            let value = self.lower_expr(arg_expr, analysis)?;
            let slot = self.alloca_for_kind(&value.kind(), param_name)?;
            self.store_value(slot, value)?;
            self.insert_var(
                param_name.clone(),
                VarInfo {
                    ptr: slot,
                    kind: value.kind(),
                },
            );
        }

        if let Some(parent) = &decl.parent {
            let SemanticType::Custom(parent_name) = SemanticType::from_type_ref(parent) else {
                return Err(format!(
                    "El padre de {} debe ser un tipo nombrado",
                    type_name
                ));
            };

            // Determine parent args to pass:
            // - If the type declaration supplies explicit parent_arg expressions, use them.
            // - Else if the child type has no explicit ctor params (implicit inheritance),
            //   forward the current `arg_exprs` (or the prefix matching parent's arity).
            // - Otherwise, pass an empty list.
            let parent_args_vec: Vec<crate::ast::Expr> = if let Some(parent_arg) = &decl.parent_arg
            {
                if !parent_arg.is_empty() {
                    parent_arg.clone()
                } else if decl.param.is_empty() {
                    // child has no explicit params -> implicit inheritance: forward the args
                    let parent_arity = analysis
                        .type_shapes
                        .get(&parent_name)
                        .map(|s| s.ctor_params.len())
                        .unwrap_or(0);
                    if parent_arity == 0 {
                        Vec::new()
                    } else if arg_exprs.len() >= parent_arity {
                        arg_exprs[..parent_arity].to_vec()
                    } else {
                        return Err(format!(
                            "Aridad invalida al construir {}: se esperaban {} argumentos para el padre {} pero llegaron {}",
                            type_name,
                            parent_arity,
                            parent_name,
                            arg_exprs.len()
                        ));
                    }
                } else {
                    Vec::new()
                }
            } else if decl.param.is_empty() {
                // No explicit parent_arg and no child params -> forward all args
                arg_exprs.to_vec()
            } else {
                Vec::new()
            };

            self.initialize_object_fields(
                concrete_type,
                &parent_name,
                &parent_args_vec,
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
        let index = self
            .object_field_names(type_name)?
            .iter()
            .position(|name| name == field_name)
            .ok_or_else(|| format!("El tipo {} no define el campo {}", type_name, field_name))?;
        Ok(index + 1)
    }

    pub(super) fn cast_object_ptr(
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

    pub(super) fn get_malloc_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("malloc") {
            return function;
        }

        let i8_ptr_type = self.context.i8_type().ptr_type(AddressSpace::default());
        let fn_type = i8_ptr_type.fn_type(&[self.context.i64_type().into()], false);
        self.module.add_function("malloc", fn_type, None)
    }

    fn load_type_id(
        &self,
        object_value: PointerValue<'ctx>,
        static_type: &str,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        let object_struct = self.object_struct_type(static_type)?;
        let typed_ptr = self.cast_object_ptr(&object_value, static_type)?;
        let vtable_slot = self
            .builder
            .build_struct_gep(object_struct, typed_ptr, 0, "vtable_slot")
            .map_err(|e| e.to_string())?;
        let vtable_ptr = self
            .builder
            .build_load(self.vtable_ptr_type(), vtable_slot, "vtable_ptr")
            .map_err(|e| e.to_string())?
            .into_pointer_value();
        let type_ptr = self
            .builder
            .build_pointer_cast(
                vtable_ptr,
                self.context.i64_type().ptr_type(AddressSpace::default()),
                "type_id_ptr",
            )
            .map_err(|e| e.to_string())?;
        let type_id = self
            .builder
            .build_load(self.context.i64_type(), type_ptr, "type_id")
            .map_err(|e| e.to_string())?
            .into_int_value();
        Ok(type_id)
    }
}
