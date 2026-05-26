use inkwell::values::{FloatValue, FunctionValue, PointerValue};
use inkwell::AddressSpace;

use crate::ast::{BaseCallExpr, CallExpr, KindExpr};
use crate::semantic::SemanticAnalysis;
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
            KindExpr::MemberAccess(member) => self.lower_method_call(call, member, analysis),
            _ => Err("Solo se soportan llamadas a funciones o metodos".to_string()),
        }
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
        }
    }

    fn lower_method_call(
        &mut self,
        call: &CallExpr,
        member: &crate::ast::MemberAccessExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let object_type = self.member_object_type(member, analysis)?;
        let owner_type = self.object_method_owner(&object_type, &member.field, analysis)?;
        let method_name = self.method_symbol_name(&owner_type, &member.field);
        let Some(info) = self.get_function(&method_name).cloned() else {
            return Err(format!("Metodo no encontrado: {}.{}", owner_type, member.field));
        };

        if call.arguments.len() + 1 != info.params.len() {
            return Err(format!("Aridad invalida en llamada a {}.{}", owner_type, member.field));
        }

        let receiver = self.lower_expr(&member.object, analysis)?.into_object()?;
        let object_struct = self.object_struct_type(&object_type)?;
        let typed_ptr = self.cast_object_ptr(&receiver, &object_type)?;
        let vtable_ptr_slot = self
            .builder
            .build_struct_gep(object_struct, typed_ptr, 0, "vtable_ptr")
            .map_err(|e| e.to_string())?;
        let vtable_ptr = self
            .builder
            .build_load(
                self.context.i8_type().ptr_type(AddressSpace::default()),
                vtable_ptr_slot,
                "vtable_load",
            )
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let slot = self.method_slot(&object_type, &member.field)? + 1;
        let order = self.method_order_for_type(&object_type)?;
        let vtable_type = self.vtable_struct_type(&object_type, &order);
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
        }
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
        let printf_fn = self.get_printf_function();

        match value {
            CodegenValue::Number(num) => {
                self.print_number(printf_fn, num)?;
                Ok(CodegenValue::Number(num))
            }
            CodegenValue::String(str_val) => {
                self.print_string(printf_fn, str_val)?;
                Ok(CodegenValue::String(str_val))
            }
            CodegenValue::Bool(bool_val) => {
                self.print_bool(printf_fn, bool_val)?;
                Ok(CodegenValue::Bool(bool_val))
            }
            CodegenValue::Object(object_val) => {
                self.print_object(argument, analysis, printf_fn, object_val)?;
                Ok(CodegenValue::Object(object_val))
            }
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

    fn print_object(
        &mut self,
        argument: &crate::ast::Expr,
        analysis: &SemanticAnalysis,
        printf_fn: FunctionValue<'ctx>,
        object_value: PointerValue<'ctx>,
    ) -> Result<(), String> {
        if let Some(SemanticType::Custom(type_name)) = analysis.inferred_types.get(&argument.id) {
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

                self.print_string(printf_fn, string_value)?;
                return Ok(());
            }

            let fallback = format!("<{}>", type_name);
            let fallback_ptr = self
                .builder
                .build_global_string_ptr(&fallback, "print_fmt_obj")
                .map_err(|e| e.to_string())?
                .as_pointer_value();
            self.print_string(printf_fn, fallback_ptr)?;
            return Ok(());
        }

        let fallback_ptr = self
            .builder
            .build_global_string_ptr("<object>", "print_fmt_obj_fallback")
            .map_err(|e| e.to_string())?
            .as_pointer_value();
        self.print_string(printf_fn, fallback_ptr)?;
        Ok(())
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
