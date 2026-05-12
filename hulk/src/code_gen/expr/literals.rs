use crate::ast::{LiteralExpr, LiteralValue, VariableExpr};

use super::super::{CodeGenerator, CodegenValue, ValueKind};

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
            _ => Err("Solo se soportan literales numericos y booleanos".to_string()),
        }
    }

    pub(super) fn lower_variable(
        &mut self,
        variable: &VariableExpr,
    ) -> Result<CodegenValue<'ctx>, String> {
        let info = self
            .lookup_var(&variable.name)
            .ok_or_else(|| format!("Variable no definida: {}", variable.name))?;
        match info.kind {
            ValueKind::Number => {
                let loaded = self
                    .builder
                    .build_load(self.f64_type, info.ptr, &format!("load_{}", variable.name))
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Number(loaded.into_float_value()))
            }
            ValueKind::Bool => {
                let loaded = self
                    .builder
                    .build_load(self.bool_type, info.ptr, &format!("load_{}", variable.name))
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Bool(loaded.into_int_value()))
            }
        }
    }
}
