use crate::ast::{AssignExpr, LetExpr, KindExpr};
use crate::semantic::SemanticAnalysis;

use super::super::{CodegenValue, CodeGenerator, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_let(
        &mut self,
        let_expr: &LetExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        self.enter_scope();

        for binding in &let_expr.bindings {
            let value = self.lower_expr(&binding.initializer, analysis)?;
            let kind = value.kind();
            let ptr = self.alloca_for_kind(&kind, &binding.name)?;
            self.store_value(ptr, value)?;

            self.insert_var(
                binding.name.clone(),
                VarInfo {
                    ptr,
                    kind,
                },
            );
        }

        let result = self.lower_expr(&let_expr.body, analysis);
        self.exit_scope();
        result
    }

    pub(super) fn lower_assign(
        &mut self,
        assign: &AssignExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let KindExpr::Variable(variable) = &assign.target.kind else {
            return Err("Asignacion solo soporta variables por ahora".to_string());
        };

        let info = self
            .lookup_var(&variable.name)
            .ok_or_else(|| format!("Variable no definida: {}", variable.name))?;
        let value = self.lower_expr(&assign.value, analysis)?;

        if value.kind() != info.kind {
            return Err(format!(
                "Tipo incompatible en asignacion a {}",
                variable.name
            ));
        }

        self.store_value(info.ptr, value)?;
        self.load_value(&info.kind, info.ptr, &format!("reload_{}", variable.name))
    }
}
