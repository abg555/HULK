use inkwell::values::{FloatValue, FunctionValue, PointerValue};
use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::types::BasicType;

use crate::ast::{BaseCallExpr, CallExpr, KindExpr};
use crate::semantic::SemanticAnalysis;
use crate::semantic::symbol_table::SymbolKind;
use crate::semantic::types::SemanticType;

use super::super::{CodegenValue, CodeGenerator};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_call(
        &mut self,
        call: &CallExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        match &call.callee.kind {
            KindExpr::Variable(callee) => match callee.name.as_str() {
                "print" => self.lower_print(call, analysis),
                "sqrt" => Ok(CodegenValue::Number(self.lower_unary_math(call, analysis, "llvm.sqrt.f64")?)),
                "sin" => Ok(CodegenValue::Number(self.lower_unary_math(call, analysis, "llvm.sin.f64")?)),
                "cos" => Ok(CodegenValue::Number(self.lower_unary_math(call, analysis, "llvm.cos.f64")?)),
                "exp" => Ok(CodegenValue::Number(self.lower_unary_math(call, analysis, "llvm.exp.f64")?)),
                "log" => Ok(CodegenValue::Number(self.lower_log(call, analysis)?)),
                "rand" => Ok(CodegenValue::Number(self.lower_rand(call)?)),
                _ => self.lower_user_call(call, analysis, &callee.name),
            },
            KindExpr::MemberAccess(member) => {
                if let Some(result) = self.lower_vector_member_call(call, member, analysis)? {
                    return Ok(result);
                }
                if let Some(result) = self.lower_namespace_member_call(call, member, analysis)? {
                    return Ok(result);
                }
                // If the callee is a function-typed field (not a virtual method), call it as
                // a raw function pointer instead of going through vtable dispatch.
                let is_fn_field = analysis
                    .inferred_types
                    .get(&member.object.id)
                    .and_then(|ty| {
                        if let SemanticType::Custom(type_name) = ty {
                            analysis.type_shapes.get(type_name.as_str())
                        } else {
                            None
                        }
                    })
                    .map(|shape| {
                        shape.fields.contains_key(&member.field)
                            && !shape.methods.contains_key(&member.field)
                    })
                    .unwrap_or(false);
                if is_fn_field {
                    if let Some(SemanticType::Function(params, ret)) =
                        analysis.inferred_types.get(&call.callee.id).cloned()
                    {
                        return self.lower_function_ptr_call(call, &params, &*ret, analysis);
                    }
                }
                self.lower_method_call(call, member, analysis)
            }
            _ => Err("Solo se soportan llamadas a funciones o metodos".to_string()),
        }
    }

    fn lower_namespace_member_call(
        &mut self,
        call: &CallExpr,
        member: &crate::ast::MemberAccessExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<Option<CodegenValue<'ctx>>, String> {
        let Some(SemanticType::Custom(namespace_name)) = analysis.inferred_types.get(&member.object.id) else {
            return Ok(None);
        };

        let is_namespace = analysis
            .global_symbols
            .get(namespace_name)
            .is_some_and(|symbol| symbol.kind == SymbolKind::Namespace);
        if !is_namespace {
            return Ok(None);
        }

        let Some(info) = self.get_function(&member.field).cloned() else {
            return Err(format!(
                "Miembro importado no disponible en codegen: {}.{}",
                namespace_name, member.field
            ));
        };

        if call.arguments.len() != info.params.len() {
            return Err(format!(
                "Aridad invalida en llamada importada a {}.{}",
                namespace_name, member.field
            ));
        }

        let mut args = Vec::with_capacity(call.arguments.len());
        for (idx, arg_expr) in call.arguments.iter().enumerate() {
            let value = self.lower_expr(arg_expr, analysis)?;
            let arg = match info.params[idx] {
                super::super::ValueKind::Number => value.into_number()?.into(),
                super::super::ValueKind::Bool => value.into_bool()?.into(),
                super::super::ValueKind::String => value.into_string()?.into(),
                super::super::ValueKind::Object => value.into_object()?.into(),
                super::super::ValueKind::Vector => value.into_vector()?.into(),
            };
            args.push(arg);
        }

        let call_value = self
            .builder
            .build_call(
                info.function,
                &args,
                &format!("call_import_{}_{}", namespace_name, member.field),
            )
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "La llamada importada no devolvio valor".to_string())?;

        let result = match info.ret {
            super::super::ValueKind::Number => CodegenValue::Number(call_value.into_float_value()),
            super::super::ValueKind::Bool => CodegenValue::Bool(call_value.into_int_value()),
            super::super::ValueKind::String => CodegenValue::String(call_value.into_pointer_value()),
            super::super::ValueKind::Object => CodegenValue::Object(call_value.into_pointer_value()),
            super::super::ValueKind::Vector => CodegenValue::Vector(call_value.into_pointer_value()),
        };

        Ok(Some(result))
    }

    pub(super) fn lower_base_call(
        &mut self,
        call: &BaseCallExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let type_name = self
            .current_type
            .clone()
            .ok_or_else(|| "base(...) solo es valido dentro de metodos".to_string())?;
        let method_name = self
            .current_method
            .clone()
            .ok_or_else(|| "base(...) solo es valido dentro de metodos".to_string())?;

        let parent_name = self
            .type_decls
            .get(&type_name)
            .and_then(|decl| decl.parent.as_ref())
            .and_then(|parent| match crate::semantic::types::SemanticType::from_type_ref(parent) {
                crate::semantic::types::SemanticType::Custom(name) => Some(name),
                _ => None,
            })
            .ok_or_else(|| format!("{} no tiene padre para base(...) ", type_name))?;

        let owner = self.object_method_owner(&parent_name, &method_name, analysis)?;
        let symbol = self.method_symbol_name(&owner, &method_name);
        let Some(info) = self.get_function(&symbol).cloned() else {
            return Err(format!("Metodo base no encontrado: {}.{}", owner, method_name));
        };

        if call.arguments.len() + 1 != info.params.len() {
            return Err(format!(
                "Aridad invalida en base(...): se esperaban {} argumentos y llegaron {}",
                info.params.len() - 1,
                call.arguments.len()
            ));
        }

        let self_info = self
            .lookup_var("self")
            .ok_or_else(|| "No se encontro self en el scope".to_string())?;
        let receiver = self
            .load_value(&self_info.kind, self_info.ptr, "load_self")?
            .into_object()?;

        let mut args = Vec::with_capacity(info.params.len());
        args.push(receiver.into());

        for (idx, arg_expr) in call.arguments.iter().enumerate() {
            let value = self.lower_expr(arg_expr, analysis)?;
            let arg = match info.params[idx + 1] {
                super::super::ValueKind::Number => value.into_number()?.into(),
                super::super::ValueKind::Bool => value.into_bool()?.into(),
                super::super::ValueKind::String => value.into_string()?.into(),
                super::super::ValueKind::Object => value.into_object()?.into(),
                super::super::ValueKind::Vector => value.into_vector()?.into(),
            };
            args.push(arg);
        }

        let call = self
            .builder
            .build_call(info.function, &args, &format!("call_base_{}", method_name))
            .map_err(|e| e.to_string())?;
        let value = call
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "La llamada base no devolvio un valor".to_string())?;

        match info.ret {
            super::super::ValueKind::Number => Ok(CodegenValue::Number(value.into_float_value())),
            super::super::ValueKind::Bool => Ok(CodegenValue::Bool(value.into_int_value())),
            super::super::ValueKind::String => Ok(CodegenValue::String(value.into_pointer_value())),
            super::super::ValueKind::Object => Ok(CodegenValue::Object(value.into_pointer_value())),
            super::super::ValueKind::Vector => Ok(CodegenValue::Vector(value.into_pointer_value())),
        }
    }

    fn lower_unary_math(
        &mut self,
        call: &CallExpr,
        analysis: &SemanticAnalysis,
        intrinsic: &str,
    ) -> Result<FloatValue<'ctx>, String> {
        if call.arguments.len() != 1 {
            return Err("Aridad invalida para funcion builtin".to_string());
        }

        let arg = self.lower_expr(&call.arguments[0], analysis)?.into_number()?;
        let function = self.get_unary_intrinsic(intrinsic);
        self.build_float_call(function, &[arg.into()], intrinsic)
    }

    fn lower_log(
        &mut self,
        call: &CallExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<FloatValue<'ctx>, String> {
        if call.arguments.len() != 2 {
            return Err("Aridad invalida para funcion builtin".to_string());
        }

        let base = self.lower_expr(&call.arguments[0], analysis)?.into_number()?;
        let value = self.lower_expr(&call.arguments[1], analysis)?.into_number()?;
        let function = self.get_unary_intrinsic("llvm.log.f64");

        let ln_base = self.build_float_call(function, &[base.into()], "llvm.log.f64")?;
        let ln_value = self.build_float_call(function, &[value.into()], "llvm.log.f64")?;

        self.builder
            .build_float_div(ln_value, ln_base, "logtmp")
            .map_err(|e| e.to_string())
    }

    fn lower_rand(&mut self, call: &CallExpr) -> Result<FloatValue<'ctx>, String> {
        if !call.arguments.is_empty() {
            return Err("Aridad invalida para funcion builtin".to_string());
        }

        let function = self.get_zero_arg_builtin("hulk_rand");
        self.build_float_call(function, &[], "hulk_rand")
    }

    fn lower_user_call(
        &mut self,
        call: &CallExpr,
        analysis: &SemanticAnalysis,
        name: &str,
    ) -> Result<CodegenValue<'ctx>, String> {
        let Some(info) = self.get_function(name).cloned() else {
            return Err(format!("Funcion no encontrada: {}", name));
        };

        if call.arguments.len() != info.params.len() {
            return Err(format!("Aridad invalida en llamada a {}", name));
        }

        let mut args = Vec::with_capacity(call.arguments.len());
        for (idx, arg_expr) in call.arguments.iter().enumerate() {
            let value = self.lower_expr(arg_expr, analysis)?;
            let arg = match info.params[idx] {
                super::super::ValueKind::Number => value.into_number()?.into(),
                super::super::ValueKind::Bool => value.into_bool()?.into(),
                super::super::ValueKind::String => value.into_string()?.into(),
                super::super::ValueKind::Object => value.into_object()?.into(),
                super::super::ValueKind::Vector => value.into_vector()?.into(),
            };
            args.push(arg);
        }

        let call = self
            .builder
            .build_call(info.function, &args, &format!("call_{}", name))
            .map_err(|e| e.to_string())?;
        let value = call
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "La llamada no devolvio un valor".to_string())?;

        match info.ret {
            super::super::ValueKind::Number => Ok(CodegenValue::Number(value.into_float_value())),
            super::super::ValueKind::Bool => Ok(CodegenValue::Bool(value.into_int_value())),
            super::super::ValueKind::String => Ok(CodegenValue::String(value.into_pointer_value())),
            super::super::ValueKind::Object => Ok(CodegenValue::Object(value.into_pointer_value())),
            super::super::ValueKind::Vector => Ok(CodegenValue::Vector(value.into_pointer_value())),
        }
    }

    fn resolve_vtable_layout_type(
        &self,
        type_name: &str,
        method_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Result<String, String> {
        if self.type_decls.contains_key(type_name) {
            return Ok(type_name.to_string());
        }
        if analysis.protocol_shapes.contains_key(type_name) {
            // Protocol type: find a concrete implementing type for vtable layout.
            // Use sorted order for determinism; any implementor is valid when the
            // method slot is consistent (e.g. functor wrappers with no parent).
            let mut candidates: Vec<&String> = self
                .type_decls
                .keys()
                .filter(|t| {
                    analysis
                        .type_shapes
                        .get(t.as_str())
                        .map(|s| s.methods.contains_key(method_name))
                        .unwrap_or(false)
                })
                .collect();
            candidates.sort();
            return candidates
                .into_iter()
                .next()
                .map(|t| t.to_string())
                .ok_or_else(|| {
                    format!(
                        "No se encontro implementacion concreta del protocolo {} con metodo {}",
                        type_name, method_name
                    )
                });
        }
        Err(format!("Tipo no definido: {}", type_name))
    }

    fn lower_function_ptr_call(
        &mut self,
        call: &CallExpr,
        params: &[crate::semantic::types::SemanticType],
        ret: &crate::semantic::types::SemanticType,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let fn_ptr = self.lower_expr(&call.callee, analysis)?.into_object()?;

        let param_kinds: Vec<super::super::ValueKind> = params
            .iter()
            .map(|p| self.value_kind_from_semantic(p))
            .collect::<Result<_, _>>()?;
        let ret_kind = self.value_kind_from_semantic(ret)?;
        let fn_type = self.fn_type_for_signature(&param_kinds, ret_kind);

        if call.arguments.len() != params.len() {
            return Err(format!(
                "Aridad invalida en llamada indirecta: esperaba {} argumentos",
                params.len()
            ));
        }

        let mut args = Vec::with_capacity(params.len());
        for (idx, arg_expr) in call.arguments.iter().enumerate() {
            let value = self.lower_expr(arg_expr, analysis)?;
            let arg = match param_kinds[idx] {
                super::super::ValueKind::Number => value.into_number()?.into(),
                super::super::ValueKind::Bool => value.into_bool()?.into(),
                super::super::ValueKind::String => value.into_string()?.into(),
                super::super::ValueKind::Object => value.into_object()?.into(),
                super::super::ValueKind::Vector => value.into_vector()?.into(),
            };
            args.push(arg);
        }

        let fn_ptr_typed = self
            .builder
            .build_pointer_cast(
                fn_ptr,
                fn_type.ptr_type(AddressSpace::default()),
                "fn_ptr_cast",
            )
            .map_err(|e| e.to_string())?;
        let call_result = self
            .builder
            .build_indirect_call(fn_type, fn_ptr_typed, &args, "indirect_fn_call")
            .map_err(|e| e.to_string())?;
        let value = call_result
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "La llamada no devolvio un valor".to_string())?;

        match ret_kind {
            super::super::ValueKind::Number => Ok(CodegenValue::Number(value.into_float_value())),
            super::super::ValueKind::Bool => Ok(CodegenValue::Bool(value.into_int_value())),
            super::super::ValueKind::String => Ok(CodegenValue::String(value.into_pointer_value())),
            super::super::ValueKind::Object => Ok(CodegenValue::Object(value.into_pointer_value())),
            super::super::ValueKind::Vector => Ok(CodegenValue::Vector(value.into_pointer_value())),
        }
    }

    fn lower_method_call(
        &mut self,
        call: &CallExpr,
        member: &crate::ast::MemberAccessExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let object_type = self.member_object_type(member, analysis)?;

        // When the static type is a protocol (not a concrete type in type_decls),
        // find a concrete implementing type to use for vtable layout.
        let layout_type =
            self.resolve_vtable_layout_type(&object_type, &member.field, analysis)?;

        let owner_type = self.object_method_owner(&layout_type, &member.field, analysis)?;
        let method_name = self.method_symbol_name(&owner_type, &member.field);
        let Some(info) = self.get_function(&method_name).cloned() else {
            return Err(format!("Metodo no encontrado: {}.{}", owner_type, member.field));
        };

        if call.arguments.len() + 1 != info.params.len() {
            return Err(format!("Aridad invalida en llamada a {}.{}", owner_type, member.field));
        }

        let receiver = self.lower_expr(&member.object, analysis)?.into_object()?;

        // Load vtable pointer: concrete types use a typed struct GEP; protocol types
        // (raw i8* with no prepared struct) interpret the pointer directly as i8**.
        let i8_ptr_type = self.context.i8_type().ptr_type(AddressSpace::default());
        let vtable_ptr = if self.struct_types.contains_key(&object_type) {
            let object_struct = self.object_struct_type(&object_type)?;
            let typed_ptr = self.cast_object_ptr(&receiver, &object_type)?;
            let vtable_ptr_slot = self
                .builder
                .build_struct_gep(object_struct, typed_ptr, 0, "vtable_ptr")
                .map_err(|e| e.to_string())?;
            self.builder
                .build_load(i8_ptr_type, vtable_ptr_slot, "vtable_load")
                .map_err(|e| e.to_string())?
                .into_pointer_value()
        } else {
            // Protocol-typed receiver: vtable pointer is always at offset 0 of any object.
            let ptr_to_vtable = self
                .builder
                .build_pointer_cast(
                    receiver,
                    i8_ptr_type.ptr_type(AddressSpace::default()),
                    "vtable_ptr_ptr",
                )
                .map_err(|e| e.to_string())?;
            self.builder
                .build_load(i8_ptr_type, ptr_to_vtable, "vtable_load")
                .map_err(|e| e.to_string())?
                .into_pointer_value()
        };

        let slot = self.method_slot(&layout_type, &member.field)? + 1;
        let order = self.method_order_for_type(&layout_type)?;
        let vtable_type = self.vtable_struct_type(&layout_type, &order);
        let vtable_typed = self
            .builder
            .build_pointer_cast(
                vtable_ptr,
                vtable_type.ptr_type(AddressSpace::default()),
                "vtable_typed",
            )
            .map_err(|e| e.to_string())?;
        let method_ptr_slot = self
            .builder
            .build_struct_gep(vtable_type, vtable_typed, slot as u32, "method_ptr")
            .map_err(|e| e.to_string())?;
        let method_ptr = self
            .builder
            .build_load(
                self.context.i8_type().ptr_type(AddressSpace::default()),
                method_ptr_slot,
                "method_load",
            )
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let mut args = Vec::with_capacity(info.params.len());
        args.push(receiver.into());

        for (idx, arg_expr) in call.arguments.iter().enumerate() {
            let value = self.lower_expr(arg_expr, analysis)?;
            let arg = match info.params[idx + 1] {
                super::super::ValueKind::Number => value.into_number()?.into(),
                super::super::ValueKind::Bool => value.into_bool()?.into(),
                super::super::ValueKind::String => value.into_string()?.into(),
                super::super::ValueKind::Object => value.into_object()?.into(),
                super::super::ValueKind::Vector => value.into_vector()?.into(),
            };
            args.push(arg);
        }

        let fn_type = self.fn_type_for_signature(&info.params, info.ret);
        let fn_ptr = self
            .builder
            .build_pointer_cast(
                method_ptr,
                fn_type.ptr_type(AddressSpace::default()),
                "method_fn",
            )
            .map_err(|e| e.to_string())?;
        let call = self
            .builder
            .build_indirect_call(fn_type, fn_ptr, &args, &format!("call_{}_{}", owner_type, member.field))
            .map_err(|e| e.to_string())?;
        let value = call
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "La llamada no devolvio un valor".to_string())?;

        match info.ret {
            super::super::ValueKind::Number => Ok(CodegenValue::Number(value.into_float_value())),
            super::super::ValueKind::Bool => Ok(CodegenValue::Bool(value.into_int_value())),
            super::super::ValueKind::String => Ok(CodegenValue::String(value.into_pointer_value())),
            super::super::ValueKind::Object => Ok(CodegenValue::Object(value.into_pointer_value())),
            super::super::ValueKind::Vector => Ok(CodegenValue::Vector(value.into_pointer_value())),
        }
    }

    fn lower_vector_member_call(
        &mut self,
        call: &CallExpr,
        member: &crate::ast::MemberAccessExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<Option<CodegenValue<'ctx>>, String> {
        let Some(SemanticType::Vector(inner)) = analysis.inferred_types.get(&member.object.id) else {
            return Ok(None);
        };
        let element_type = *inner.clone();

        match member.field.as_str() {
            "size" => {
                if !call.arguments.is_empty() {
                    return Err("size() no recibe argumentos".to_string());
                }
                let vec_ptr = self.lower_expr(&member.object, analysis)?.into_vector()?;
                let len_slot = self
                    .builder
                    .build_struct_gep(self.vector_struct, vec_ptr, 0, "vec_len_slot")
                    .map_err(|e| e.to_string())?;
                let len_val = self
                    .builder
                    .build_load(self.context.i64_type(), len_slot, "vec_len")
                    .map_err(|e| e.to_string())?
                    .into_int_value();
                let len_f64 = self
                    .builder
                    .build_unsigned_int_to_float(len_val, self.f64_type, "vec_len_f64")
                    .map_err(|e| e.to_string())?;
                Ok(Some(CodegenValue::Number(len_f64)))
            }
            "next" => {
                if !call.arguments.is_empty() {
                    return Err("next() no recibe argumentos".to_string());
                }
                let vec_ptr = self.lower_expr(&member.object, analysis)?.into_vector()?;
                let has_next = self.vector_next(vec_ptr)?;
                Ok(Some(CodegenValue::Bool(has_next)))
            }
            "current" => {
                if !call.arguments.is_empty() {
                    return Err("current() no recibe argumentos".to_string());
                }
                let vec_ptr = self.lower_expr(&member.object, analysis)?.into_vector()?;

                Ok(Some(self.vector_current(vec_ptr, &element_type)?))
            }
            other => Err(format!("Vector no soporta el miembro {}", other)),
        }
    }

    pub(super) fn vector_next(
        &mut self,
        vec_ptr: PointerValue<'ctx>,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        let len_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_ptr, 0, "vec_len_slot")
            .map_err(|e| e.to_string())?;
        let len_val = self
            .builder
            .build_load(self.context.i64_type(), len_slot, "vec_len")
            .map_err(|e| e.to_string())?
            .into_int_value();

        let cursor_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_ptr, 2, "vec_cursor_slot")
            .map_err(|e| e.to_string())?;
        let cursor_val = self
            .builder
            .build_load(self.context.i64_type(), cursor_slot, "vec_cursor")
            .map_err(|e| e.to_string())?
            .into_int_value();
        let next_cursor = self
            .builder
            .build_int_add(
                cursor_val,
                self.context.i64_type().const_int(1, false),
                "vec_next_cursor",
            )
            .map_err(|e| e.to_string())?;
        let has_next = self
            .builder
            .build_int_compare(IntPredicate::ULT, next_cursor, len_val, "vec_has_next")
            .map_err(|e| e.to_string())?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .ok_or_else(|| "No se pudo determinar la funcion actual para next(vector)".to_string())?;
        let has_next_block = self.context.append_basic_block(function, "vec_next_true");
        let no_next_block = self.context.append_basic_block(function, "vec_next_false");
        let cont_block = self.context.append_basic_block(function, "vec_next_cont");
        self.builder
            .build_conditional_branch(has_next, has_next_block, no_next_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(has_next_block);
        self.builder
            .build_store(cursor_slot, next_cursor)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(cont_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(no_next_block);
        self.builder
            .build_store(cursor_slot, len_val)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(cont_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cont_block);
        Ok(has_next)
    }

    pub(super) fn vector_current(
        &mut self,
        vec_ptr: PointerValue<'ctx>,
        element_type: &SemanticType,
    ) -> Result<CodegenValue<'ctx>, String> {
        let len_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_ptr, 0, "vec_len_slot")
            .map_err(|e| e.to_string())?;
        let len_val = self
            .builder
            .build_load(self.context.i64_type(), len_slot, "vec_len")
            .map_err(|e| e.to_string())?
            .into_int_value();

        let cursor_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_ptr, 2, "vec_cursor_slot")
            .map_err(|e| e.to_string())?;
        let cursor_val = self
            .builder
            .build_load(self.context.i64_type(), cursor_slot, "vec_cursor")
            .map_err(|e| e.to_string())?
            .into_int_value();

        let has_current = self
            .builder
            .build_int_compare(IntPredicate::ULT, cursor_val, len_val, "vec_has_current")
            .map_err(|e| e.to_string())?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .ok_or_else(|| "No se pudo determinar la funcion actual para current(vector)".to_string())?;
        let ok_block = self.context.append_basic_block(function, "vec_current_ok");
        let fail_block = self.context.append_basic_block(function, "vec_current_fail");
        self.builder
            .build_conditional_branch(has_current, ok_block, fail_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(fail_block);
        let panic_fn = self.get_panic_function();
        let msg_name = self.fresh_tmp("vec_current_panic_msg");
        let msg = self
            .builder
            .build_global_string_ptr(
                "Runtime error: current() sobre vector vacio",
                &msg_name,
            )
            .map_err(|e| e.to_string())?;
        self.builder
            .build_call(panic_fn, &[msg.as_pointer_value().into()], "vec_current_panic")
            .map_err(|e| e.to_string())?;
        self.builder.build_unreachable().map_err(|e| e.to_string())?;

        self.builder.position_at_end(ok_block);

        let data_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_ptr, 1, "vec_data_slot")
            .map_err(|e| e.to_string())?;
        let data_i8 = self
            .builder
            .build_load(
                self.context.i8_type().ptr_type(AddressSpace::default()),
                data_slot,
                "vec_data_ptr",
            )
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let elem_basic = self.basic_type_for_semantic(element_type)?;
        let elem_size = elem_basic
            .size_of()
            .ok_or_else(|| "No se pudo calcular tamano del elemento".to_string())?;
        let elem_size_i64 = self
            .builder
            .build_int_cast(elem_size, self.context.i64_type(), "vec_current_elem_size")
            .map_err(|e| e.to_string())?;
        let elem_ptr_type = match elem_basic {
            inkwell::types::BasicTypeEnum::FloatType(ft) => ft.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::IntType(it) => it.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::PointerType(pt) => {
                pt.ptr_type(AddressSpace::default())
            }
            inkwell::types::BasicTypeEnum::StructType(st) => st.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::ArrayType(at) => at.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::VectorType(vt) => vt.ptr_type(AddressSpace::default()),
        };

        let byte_offset = self
            .builder
            .build_int_mul(cursor_val, elem_size_i64, "vec_current_byte_offset")
            .map_err(|e| e.to_string())?;
        let elem_i8_ptr = unsafe {
            self.builder
                .build_in_bounds_gep(
                    self.context.i8_type(),
                    data_i8,
                    &[byte_offset],
                    "vec_current_elem_i8",
                )
                .map_err(|e| e.to_string())?
        };
        let elem_ptr = self
            .builder
            .build_pointer_cast(elem_i8_ptr, elem_ptr_type, "vec_current_ptr")
            .map_err(|e| e.to_string())?;

        let elem_kind = self.value_kind_from_semantic(element_type)?;
        let current_value = match elem_kind {
            super::super::ValueKind::Number => CodegenValue::Number(
                self.builder
                    .build_load(self.f64_type, elem_ptr, "vec_current_num")
                    .map_err(|e| e.to_string())?
                    .into_float_value(),
            ),
            super::super::ValueKind::Bool => CodegenValue::Bool(
                self.builder
                    .build_load(self.bool_type, elem_ptr, "vec_current_bool")
                    .map_err(|e| e.to_string())?
                    .into_int_value(),
            ),
            super::super::ValueKind::String => CodegenValue::String(
                self.builder
                    .build_load(
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "vec_current_str",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            super::super::ValueKind::Object => CodegenValue::Object(
                self.builder
                    .build_load(
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "vec_current_obj",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            super::super::ValueKind::Vector => CodegenValue::Vector(
                self.builder
                    .build_load(
                        self.vector_struct.ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "vec_current_vec",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
        };

        Ok(current_value)
    }

    fn lower_print(
        &mut self,
        call: &CallExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        if call.arguments.len() != 1 {
            return Err("Aridad invalida para funcion builtin".to_string());
        }

        let argument = &call.arguments[0];
        let value = self.lower_expr(argument, analysis)?;
        let semantic_type = analysis
            .inferred_types
            .get(&argument.id)
            .ok_or_else(|| "No se encontro el tipo inferido para print".to_string())?;
        let printf_fn = self.get_printf_function();
        let string_value = self.value_to_string(value, semantic_type, analysis)?;

        self.print_string(printf_fn, string_value)?;

        match value {
            CodegenValue::Number(num) => {
                Ok(CodegenValue::Number(num))
            }
            CodegenValue::String(str_val) => {
                Ok(CodegenValue::String(str_val))
            }
            CodegenValue::Bool(bool_val) => {
                Ok(CodegenValue::Bool(bool_val))
            }
            CodegenValue::Object(object_val) => {
                Ok(CodegenValue::Object(object_val))
            }
            CodegenValue::Vector(vector_val) => Ok(CodegenValue::Vector(vector_val)),
        }
    }

    fn print_number(
        &self,
        printf_fn: FunctionValue<'ctx>,
        value: inkwell::values::FloatValue<'ctx>,
    ) -> Result<(), String> {
        let fmt = self
            .builder
            .build_global_string_ptr("%f\n", "print_fmt_num")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_call(printf_fn, &[fmt.as_pointer_value().into(), value.into()], "print")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn print_string(
        &self,
        printf_fn: FunctionValue<'ctx>,
        value: PointerValue<'ctx>,
    ) -> Result<(), String> {
        let fmt = self
            .builder
            .build_global_string_ptr("%s\n", "print_fmt_str")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_call(printf_fn, &[fmt.as_pointer_value().into(), value.into()], "print")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn print_bool(
        &self,
        printf_fn: FunctionValue<'ctx>,
        value: inkwell::values::IntValue<'ctx>,
    ) -> Result<(), String> {
        let fmt = self
            .builder
            .build_global_string_ptr("%d\n", "print_fmt_bool")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_call(printf_fn, &[fmt.as_pointer_value().into(), value.into()], "print")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn value_to_string(
        &mut self,
        value: CodegenValue<'ctx>,
        semantic_type: &SemanticType,
        analysis: &SemanticAnalysis,
    ) -> Result<PointerValue<'ctx>, String> {
        match (semantic_type, value) {
            (SemanticType::Number, CodegenValue::Number(number)) => {
                let fmt_fn = self.get_format_number_function();
                let call = self
                    .builder
                    .build_call(fmt_fn, &[number.into()], "format_num")
                    .map_err(|e| e.to_string())?;

                let value = call.try_as_basic_value()
                    .left()
                    .ok_or_else(|| "format_number no devolvio un valor".to_string())?
                    .into_pointer_value();
                Ok(value)
            }
            (SemanticType::Boolean, CodegenValue::Bool(boolean)) => {
                let false_value = self.bool_type.const_zero();
                let is_false = self
                    .builder
                    .build_int_compare(IntPredicate::EQ, boolean, false_value, "bool_is_false")
                    .map_err(|e| e.to_string())?;
                let true_name = self.fresh_tmp("bool_true");
                let true_ptr = self
                    .builder
                    .build_global_string_ptr("true", &true_name)
                    .map_err(|e| e.to_string())?
                    .as_pointer_value();
                let false_name = self.fresh_tmp("bool_false");
                let false_ptr = self
                    .builder
                    .build_global_string_ptr("false", &false_name)
                    .map_err(|e| e.to_string())?
                    .as_pointer_value();
                let selected = self
                    .builder
                    .build_select(is_false, false_ptr, true_ptr, "bool_str")
                    .map_err(|e| e.to_string())?
                    .into_pointer_value();
                Ok(selected)
            }
            (SemanticType::String, CodegenValue::String(string_value)) => Ok(string_value),
            (SemanticType::Custom(type_name), CodegenValue::Object(object_value)) => {
                self.object_to_string(type_name, object_value, analysis)
            }
            (SemanticType::Vector(inner), CodegenValue::Vector(vector_value)) => {
                self.vector_to_string(vector_value, inner, analysis)
            }
            (expected, actual) => Err(format!(
                "No se pudo convertir a string: se esperaba {} y se obtuvo {:?}",
                expected, actual.kind()
            )),
        }
    }

    fn object_to_string(
        &mut self,
        type_name: &str,
        object_value: PointerValue<'ctx>,
        analysis: &SemanticAnalysis,
    ) -> Result<PointerValue<'ctx>, String> {
        if let Ok(owner_type) = self.object_method_owner(type_name, "toString", analysis) {
            let symbol = self.method_symbol_name(&owner_type, "toString");
            let Some(info) = self.get_function(&symbol).cloned() else {
                return Err(format!("Metodo no encontrado: {}.toString", owner_type));
            };

            if info.params.len() != 1 || info.ret != super::super::ValueKind::String {
                return Err(format!(
                    "toString de {} debe tener firma toString(): String",
                    owner_type
                ));
            }

            let call = self
                .builder
                .build_call(info.function, &[object_value.into()], "call_toString")
                .map_err(|e| e.to_string())?;
            let string_value = call
                .try_as_basic_value()
                .left()
                .ok_or_else(|| "toString no devolvio un valor".to_string())?
                .into_pointer_value();

            return Ok(string_value);
        }

        let fallback = format!("<{}>", type_name);
        let fallback_name = self.fresh_tmp("obj_str");
        let fallback_ptr = self
            .builder
            .build_global_string_ptr(&fallback, &fallback_name)
            .map_err(|e| e.to_string())?
            .as_pointer_value();
        Ok(fallback_ptr)
    }

    fn vector_to_string(
        &mut self,
        vector_value: PointerValue<'ctx>,
        inner: &SemanticType,
        analysis: &SemanticAnalysis,
    ) -> Result<PointerValue<'ctx>, String> {
        let len_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vector_value, 0, "vec_str_len_slot")
            .map_err(|e| e.to_string())?;
        let len_val = self
            .builder
            .build_load(self.context.i64_type(), len_slot, "vec_str_len")
            .map_err(|e| e.to_string())?
            .into_int_value();

        let data_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vector_value, 1, "vec_str_data_slot")
            .map_err(|e| e.to_string())?;
        let data_ptr = self
            .builder
            .build_load(
                self.context.i8_type().ptr_type(AddressSpace::default()),
                data_slot,
                "vec_str_data",
            )
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let elem_kind = self.value_kind_from_semantic(inner)?;
        let elem_basic = self.basic_type_for_semantic(inner)?;
        let elem_size = elem_basic
            .size_of()
            .ok_or_else(|| "No se pudo calcular tamano del elemento".to_string())?;
        let elem_size_i64 = self
            .builder
            .build_int_cast(elem_size, self.context.i64_type(), "vec_str_elem_size")
            .map_err(|e| e.to_string())?;

        let open_name = self.fresh_tmp("vec_open");
        let open_ptr = self
            .builder
            .build_global_string_ptr("[", &open_name)
            .map_err(|e| e.to_string())?
            .as_pointer_value();
        let close_name = self.fresh_tmp("vec_close");
        let close_ptr = self
            .builder
            .build_global_string_ptr("]", &close_name)
            .map_err(|e| e.to_string())?
            .as_pointer_value();
        let sep_name = self.fresh_tmp("vec_sep");
        let sep_ptr = self
            .builder
            .build_global_string_ptr(", ", &sep_name)
            .map_err(|e| e.to_string())?
            .as_pointer_value();

        let result_slot = self.alloca_for_kind(&super::super::ValueKind::String, "vec_render")?;
        self.store_value(result_slot, CodegenValue::String(open_ptr))?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .ok_or_else(|| "No se pudo determinar la funcion actual para print(vector)".to_string())?;
        let loop_cond = self.context.append_basic_block(function, "vec_render_cond");
        let loop_body = self.context.append_basic_block(function, "vec_render_body");
        let loop_after = self.context.append_basic_block(function, "vec_render_after");

        let idx_slot = self
            .builder
            .build_alloca(self.context.i64_type(), "vec_render_idx")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(idx_slot, self.context.i64_type().const_int(0, false))
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(loop_cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(loop_cond);
        let idx_val = self
            .builder
            .build_load(self.context.i64_type(), idx_slot, "vec_render_idx_val")
            .map_err(|e| e.to_string())?
            .into_int_value();
        let continue_loop = self
            .builder
            .build_int_compare(IntPredicate::ULT, idx_val, len_val, "vec_render_cmp")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_conditional_branch(continue_loop, loop_body, loop_after)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(loop_body);
        let byte_offset = self
            .builder
            .build_int_mul(idx_val, elem_size_i64, "vec_render_byte_offset")
            .map_err(|e| e.to_string())?;
        let data_i8 = self
            .builder
            .build_pointer_cast(
                data_ptr,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "vec_render_data_i8",
            )
            .map_err(|e| e.to_string())?;
        let elem_i8 = unsafe {
            self.builder
                .build_in_bounds_gep(
                    self.context.i8_type(),
                    data_i8,
                    &[byte_offset],
                    "vec_render_elem_i8",
                )
                .map_err(|e| e.to_string())?
        };
        let elem_ptr = self
            .builder
            .build_pointer_cast(elem_i8, elem_basic.ptr_type(AddressSpace::default()), "vec_render_elem_ptr")
            .map_err(|e| e.to_string())?;

        let element_value = match elem_kind {
            super::super::ValueKind::Number => CodegenValue::Number(
                self.builder
                    .build_load(self.f64_type, elem_ptr, "vec_render_elem_num")
                    .map_err(|e| e.to_string())?
                    .into_float_value(),
            ),
            super::super::ValueKind::Bool => CodegenValue::Bool(
                self.builder
                    .build_load(self.bool_type, elem_ptr, "vec_render_elem_bool")
                    .map_err(|e| e.to_string())?
                    .into_int_value(),
            ),
            super::super::ValueKind::String => CodegenValue::String(
                self.builder
                    .build_load(
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "vec_render_elem_str",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            super::super::ValueKind::Object => CodegenValue::Object(
                self.builder
                    .build_load(
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "vec_render_elem_obj",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            super::super::ValueKind::Vector => CodegenValue::Vector(
                self.builder
                    .build_load(
                        self.vector_struct.ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "vec_render_elem_vec",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
        };

        let element_string = self.value_to_string(element_value, inner, analysis)?;

        let separator_needed = self
            .builder
            .build_int_compare(IntPredicate::NE, idx_val, self.context.i64_type().const_int(0, false), "vec_render_need_sep")
            .map_err(|e| e.to_string())?;
        let sep_block = self.context.append_basic_block(function, "vec_render_sep");
        let append_block = self.context.append_basic_block(function, "vec_render_append");
        self.builder
            .build_conditional_branch(separator_needed, sep_block, append_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(sep_block);
        let current = self
            .load_value(&super::super::ValueKind::String, result_slot, "vec_render_current")?
            .into_string()?;
        let with_sep = self.concat_strings(current, sep_ptr)?;
        self.store_value(result_slot, CodegenValue::String(with_sep))?;
        self.builder
            .build_unconditional_branch(append_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(append_block);
        let current = self
            .load_value(&super::super::ValueKind::String, result_slot, "vec_render_current2")?
            .into_string()?;
        let appended = self.concat_strings(current, element_string)?;
        self.store_value(result_slot, CodegenValue::String(appended))?;

        let next_idx = self
            .builder
            .build_int_add(idx_val, self.context.i64_type().const_int(1, false), "vec_render_next")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(idx_slot, next_idx)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(loop_cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(loop_after);
        let current = self
            .load_value(&super::super::ValueKind::String, result_slot, "vec_render_done")?
            .into_string()?;
        let closed = self.concat_strings(current, close_ptr)?;
        Ok(closed)
    }

    fn concat_strings(
        &self,
        left: PointerValue<'ctx>,
        right: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, String> {
        let concat_fn = self.get_concat_function();
        let call = self
            .builder
            .build_call(concat_fn, &[left.into(), right.into()], "concat")
            .map_err(|e| e.to_string())?;

        let result = call.try_as_basic_value()
            .left()
            .ok_or_else(|| "concat no devolvio un valor".to_string())?
            .into_pointer_value();
        Ok(result)
    }

    fn get_unary_intrinsic(&self, name: &str) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function(name) {
            return function;
        }

        let fn_type = self.f64_type.fn_type(&[self.f64_type.into()], false);
        self.module.add_function(name, fn_type, None)
    }

    fn get_zero_arg_builtin(&self, name: &str) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function(name) {
            return function;
        }

        let fn_type = self.f64_type.fn_type(&[], false);
        self.module.add_function(name, fn_type, None)
    }

    fn get_printf_function(&self) -> FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("printf") {
            return function;
        }

        let i8_ptr = self
            .context
            .i8_type()
            .ptr_type(AddressSpace::default());
        let fn_type = self.context.i32_type().fn_type(&[i8_ptr.into()], true);
        self.module.add_function("printf", fn_type, None)
    }

    fn build_float_call(
        &self,
        function: FunctionValue<'ctx>,
        args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
        name: &str,
    ) -> Result<FloatValue<'ctx>, String> {
        let call = self
            .builder
            .build_call(function, args, name)
            .map_err(|e| e.to_string())?;

        let value = call
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "La llamada no devolvio un valor".to_string())?;

        Ok(value.into_float_value())
    }
}
