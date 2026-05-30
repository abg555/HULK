use crate::ast::BlockExpr;
use crate::semantic::SemanticAnalysis;

use super::super::{CodegenValue, CodeGenerator};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_block(
        &mut self,
        block: &BlockExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        if block.expressions.is_empty() {
            return Err("Bloque vacio sin valor".to_string());
        }

        let mut last_value = None;
        for expr in &block.expressions {
            last_value = Some(self.lower_expr(expr, analysis)?);
        }

        last_value.ok_or_else(|| "Bloque vacio sin valor".to_string())
    }
}
