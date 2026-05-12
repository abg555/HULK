use inkwell::values::{FloatValue, FunctionValue};
use inkwell::AddressSpace;

use crate::ast::{CallExpr, KindExpr};
use crate::semantic::SemanticAnalysis;

use super::super::{CodegenValue, CodeGenerator};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_call(
        &mut self,
        call: &CallExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let KindExpr::Variable(callee) = &call.callee.kind else {
            return Err("Solo se soportan llamadas a funciones builtin".to_string());
        };

        match callee.name.as_str() {
            "print" => Ok(CodegenValue::Number(self.lower_print(call, analysis)?)),
            "sqrt" => Ok(CodegenValue::Number(self.lower_unary_math(call, analysis, "llvm.sqrt.f64")?)),
            "sin" => Ok(CodegenValue::Number(self.lower_unary_math(call, analysis, "llvm.sin.f64")?)),
            "cos" => Ok(CodegenValue::Number(self.lower_unary_math(call, analysis, "llvm.cos.f64")?)),
            "exp" => Ok(CodegenValue::Number(self.lower_unary_math(call, analysis, "llvm.exp.f64")?)),
            "log" => Ok(CodegenValue::Number(self.lower_log(call, analysis)?)),
            "rand" => Ok(CodegenValue::Number(self.lower_rand(call)?)),
            _ => self.lower_user_call(call, analysis, &callee.name),
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
        let Some(info) = self.get_function(name) else {
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
        }
    }

    fn lower_print(
        &mut self,
        call: &CallExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<FloatValue<'ctx>, String> {
        if call.arguments.len() != 1 {
            return Err("Aridad invalida para funcion builtin".to_string());
        }

        let value = self.lower_expr(&call.arguments[0], analysis)?.into_number()?;
        let printf_fn = self.get_printf_function();
        let fmt = self
            .builder
            .build_global_string_ptr("%f\n", "print_fmt")
            .map_err(|e| e.to_string())?;

        self.builder
            .build_call(printf_fn, &[fmt.as_pointer_value().into(), value.into()], "print")
            .map_err(|e| e.to_string())?;

        Ok(value)
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
