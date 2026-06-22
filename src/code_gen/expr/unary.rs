use crate::ast::{UnaryExpr, UnaryOperator};
use crate::semantic::SemanticAnalysis;

use super::super::{CodeGenerator, CodegenValue};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_unary(
        &mut self,
        unary: &UnaryExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        match unary.operator {
            UnaryOperator::Negate => {
                let value = self.lower_expr(&unary.right, analysis)?.into_number()?;
                let result = self
                    .builder
                    .build_float_neg(value, "negtmp")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Number(result))
            }
            UnaryOperator::Not => {
                let value = self.lower_expr(&unary.right, analysis)?.into_bool()?;
                let result = self
                    .builder
                    .build_not(value, "nottmp")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Bool(result))
            }
        }
    }
}
