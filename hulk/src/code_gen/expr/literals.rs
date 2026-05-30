use crate::ast::{LiteralExpr, LiteralValue, VariableExpr};

use super::super::{CodeGenerator, CodegenValue};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_literal(
        &self,
        literal: &LiteralExpr,
    ) -> Result<CodegenValue<'ctx>, String> {
        match literal.value {
            LiteralValue::Number(value) => Ok(CodegenValue::Number(self.f64_type.const_float(value))),
            LiteralValue::Bool(value) => Ok(CodegenValue::Bool(
                self.bool_type.const_int(u64::from(value), false),
            )),
            LiteralValue::String(ref s) => {
                let global_str = self
                    .builder
                    .build_global_string_ptr(s, "str")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::String(global_str.as_pointer_value()))
            }
        }
    }

    pub(super) fn lower_variable(
        &mut self,
        variable: &VariableExpr,
    ) -> Result<CodegenValue<'ctx>, String> {
        match variable.name.as_str() {
            "PI" | "pi" => {
                return Ok(CodegenValue::Number(self.f64_type.const_float(std::f64::consts::PI)))
            }
            "E" => return Ok(CodegenValue::Number(self.f64_type.const_float(std::f64::consts::E))),
            _ => {}
        }

        let info = self
            .lookup_var(&variable.name)
            .ok_or_else(|| format!("Variable no definida: {}", variable.name))?;
        self.load_value(&info.kind, info.ptr, &format!("load_{}", variable.name))
    }
}
