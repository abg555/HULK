use crate::ast::MacroCallExpr;
use crate::semantic::SemanticAnalysis;

use super::super::{CodeGenerator, CodegenValue};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_macro_call(
        &mut self,
        call: &MacroCallExpr,
        _analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        Err(format!(
            "MacroCall '{}' llego a codegen sin expandir. Las macros son de compilacion y deben expandirse antes de codegen.",
            call.name
        ))
    }
}
