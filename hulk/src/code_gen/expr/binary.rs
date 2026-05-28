use inkwell::values::FloatValue;
use inkwell::FloatPredicate;

use crate::ast::BinaryOperator;

use super::super::{CodeGenerator, CodegenValue};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn build_binary_op(
        &self,
        operator: &BinaryOperator,
        left: CodegenValue<'ctx>,
        right: CodegenValue<'ctx>,
    ) -> Result<CodegenValue<'ctx>, String> {
        match operator {
            BinaryOperator::Add => {
                let left = left.into_number()?;
                let right = right.into_number()?;
                let value = self
                    .builder
                    .build_float_add(left, right, "addtmp")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Number(value))
            }
            BinaryOperator::Sub => {
                let left = left.into_number()?;
                let right = right.into_number()?;
                let value = self
                    .builder
                    .build_float_sub(left, right, "subtmp")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Number(value))
            }
            BinaryOperator::Mul => {
                let left = left.into_number()?;
                let right = right.into_number()?;
                let value = self
                    .builder
                    .build_float_mul(left, right, "multmp")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Number(value))
            }
            BinaryOperator::Div => {
                let left = left.into_number()?;
                let right = right.into_number()?;
                let value = self
                    .builder
                    .build_float_div(left, right, "divtmp")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Number(value))
            }
            BinaryOperator::Mod => {
                let left = left.into_number()?;
                let right = right.into_number()?;
                let value = self
                    .builder
                    .build_float_rem(left, right, "modtmp")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Number(value))
            }
            BinaryOperator::Pow => {
                let left = left.into_number()?;
                let right = right.into_number()?;
                let value = self.build_pow(left, right)?;
                Ok(CodegenValue::Number(value))
            }
            BinaryOperator::Equal => self.build_float_compare(FloatPredicate::OEQ, left, right),
            BinaryOperator::NotEqual => self.build_float_compare(FloatPredicate::ONE, left, right),
            BinaryOperator::Less => self.build_float_compare(FloatPredicate::OLT, left, right),
            BinaryOperator::Greater => self.build_float_compare(FloatPredicate::OGT, left, right),
            BinaryOperator::LessEqual => self.build_float_compare(FloatPredicate::OLE, left, right),
            BinaryOperator::GreaterEqual => self.build_float_compare(FloatPredicate::OGE, left, right),
            BinaryOperator::And => self.build_bool_op("andtmp", left, right, true),
            BinaryOperator::Or => self.build_bool_op("ortmp", left, right, false),
            BinaryOperator::Concat => self.build_string_concat(left, right),
            BinaryOperator::FullConcat => self.build_string_concat_full(left, right),
        }
    }

    fn build_bool_op(
        &self,
        name: &str,
        left: CodegenValue<'ctx>,
        right: CodegenValue<'ctx>,
        is_and: bool,
    ) -> Result<CodegenValue<'ctx>, String> {
        let left = left.into_bool()?;
        let right = right.into_bool()?;
        let value = if is_and {
            self.builder
                .build_and(left, right, name)
                .map_err(|e| e.to_string())?
        } else {
            self.builder
                .build_or(left, right, name)
                .map_err(|e| e.to_string())?
        };

        Ok(CodegenValue::Bool(value))
    }

    fn build_float_compare(
        &self,
        predicate: FloatPredicate,
        left: CodegenValue<'ctx>,
        right: CodegenValue<'ctx>,
    ) -> Result<CodegenValue<'ctx>, String> {
        let left = left.into_number()?;
        let right = right.into_number()?;
        let value = self
            .builder
            .build_float_compare(predicate, left, right, "cmptmp")
            .map_err(|e| e.to_string())?;

        Ok(CodegenValue::Bool(value))
    }

    fn build_pow(
        &self,
        left: FloatValue<'ctx>,
        right: FloatValue<'ctx>,
    ) -> Result<FloatValue<'ctx>, String> {
        let pow_fn = self.get_pow_function();
        let call = self
            .builder
            .build_call(pow_fn, &[left.into(), right.into()], "powtmp")
            .map_err(|e| e.to_string())?;

        let value = call
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "llvm.pow no devolvio un valor".to_string())?;

        Ok(value.into_float_value())
    }

    fn get_pow_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("llvm.pow.f64") {
            return function;
        }

        let fn_type = self
            .f64_type
            .fn_type(&[self.f64_type.into(), self.f64_type.into()], false);
        self.module.add_function("llvm.pow.f64", fn_type, None)
    }

    fn build_string_concat(
        &self,
        left: CodegenValue<'ctx>,
        right: CodegenValue<'ctx>,
    ) -> Result<CodegenValue<'ctx>, String> {
        // Determine operand types and convert as needed
        let left_str = match left {
            CodegenValue::String(s) => s,
            CodegenValue::Number(n) => {
                // number @ ... -> convert number to string using runtime helper
                let fmt_fn = self.get_format_number_function();
                let call = self
                    .builder
                    .build_call(fmt_fn, &[n.into()], "format_num")
                    .map_err(|e| e.to_string())?;

                let ptr = call
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| "format_number no devolvio un valor".to_string())?
                    .into_pointer_value();

                ptr
            },
            _ => return Err("Se esperaba string o numero para concatenacion".to_string()),
        };

        let right_str = match right {
            CodegenValue::String(s) => s,
            CodegenValue::Number(n) => {
                // ... @ number -> convert number to string using runtime helper
                let fmt_fn = self.get_format_number_function();
                let call = self
                    .builder
                    .build_call(fmt_fn, &[n.into()], "format_num")
                    .map_err(|e| e.to_string())?;

                let ptr = call
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| "format_number no devolvio un valor".to_string())?
                    .into_pointer_value();

                ptr
            },
            _ => return Err("Se esperaba string o numero para concatenacion".to_string()),
        };

        let concat_fn = self.get_concat_function();
        let result = self
            .builder
            .build_call(concat_fn, &[left_str.into(), right_str.into()], "concat")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "concat no devolvio un valor".to_string())?
            .into_pointer_value();

        Ok(CodegenValue::String(result))
    }

    fn build_string_concat_full(
        &self,
        left: CodegenValue<'ctx>,
        right: CodegenValue<'ctx>,
    ) -> Result<CodegenValue<'ctx>, String> {
        let left_str = match left {
            CodegenValue::String(s) => s,
            CodegenValue::Number(n) => {
                let fmt_fn = self.get_format_number_function();
                let call = self
                    .builder
                    .build_call(fmt_fn, &[n.into()], "format_num")
                    .map_err(|e| e.to_string())?;

                call.try_as_basic_value()
                    .left()
                    .ok_or_else(|| "format_number no devolvio un valor".to_string())?
                    .into_pointer_value()
            }
            _ => return Err("Se esperaba string o numero para concatenacion".to_string()),
        };

        let right_str = match right {
            CodegenValue::String(s) => s,
            CodegenValue::Number(n) => {
                let fmt_fn = self.get_format_number_function();
                let call = self
                    .builder
                    .build_call(fmt_fn, &[n.into()], "format_num")
                    .map_err(|e| e.to_string())?;

                call.try_as_basic_value()
                    .left()
                    .ok_or_else(|| "format_number no devolvio un valor".to_string())?
                    .into_pointer_value()
            }
            _ => return Err("Se esperaba string o numero para concatenacion".to_string()),
        };

        let concat_fn = self.get_concat_full_function();
        let result = self
            .builder
            .build_call(concat_fn, &[left_str.into(), right_str.into()], "concat_full")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "concat_full no devolvio un valor".to_string())?
            .into_pointer_value();

        Ok(CodegenValue::String(result))
    }

    fn get_sprintf_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("sprintf") {
            return function;
        }

        let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
        let fn_type = self.context.i32_type().fn_type(
            &[i8_ptr_type.into(), i8_ptr_type.into(), self.f64_type.into()],
            false,
        );
        self.module.add_function("sprintf", fn_type, None)
    }

    pub(super) fn get_concat_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("hulk_concat") {
            return function;
        }

        let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
        let fn_type = i8_ptr_type.fn_type(&[i8_ptr_type.into(), i8_ptr_type.into()], false);
        self.module.add_function("hulk_concat", fn_type, None)
    }

    pub(super) fn get_concat_full_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("hulk_concat_full") {
            return function;
        }

        let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
        let fn_type = i8_ptr_type.fn_type(&[i8_ptr_type.into(), i8_ptr_type.into()], false);
        self.module.add_function("hulk_concat_full", fn_type, None)
    }

    pub(super) fn get_format_number_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(function) = self.module.get_function("hulk_format_number") {
            return function;
        }

        let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
        let fn_type = i8_ptr_type.fn_type(&[self.f64_type.into()], false);
        self.module.add_function("hulk_format_number", fn_type, None)
    }
}
