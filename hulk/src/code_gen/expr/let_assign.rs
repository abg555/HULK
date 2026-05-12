use crate::ast::{AssignExpr, LetExpr, KindExpr};
use crate::semantic::SemanticAnalysis;

use super::super::{CodegenValue, CodeGenerator, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_let(
        &mut self,
        let_expr: &LetExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        self.enter_scope();

        for binding in &let_expr.bindings {
            let value = self.lower_expr(&binding.initializer, analysis)?;
            let (ptr, kind) = match value {
                CodegenValue::Number(number) => {
                    let ptr = self
                        .builder
                        .build_alloca(self.f64_type, &binding.name)
                        .map_err(|e| e.to_string())?;
                    self.builder
                        .build_store(ptr, number)
                        .map_err(|e| e.to_string())?;
                    (ptr, ValueKind::Number)
                }
                CodegenValue::Bool(bool_value) => {
                    let ptr = self
                        .builder
                        .build_alloca(self.bool_type, &binding.name)
                        .map_err(|e| e.to_string())?;
                    self.builder
                        .build_store(ptr, bool_value)
                        .map_err(|e| e.to_string())?;
                    (ptr, ValueKind::Bool)
                }
            };

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

        match value {
            CodegenValue::Number(number) => {
                self.builder
                    .build_store(info.ptr, number)
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Number(number))
            }
            CodegenValue::Bool(bool_value) => {
                self.builder
                    .build_store(info.ptr, bool_value)
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Bool(bool_value))
            }
        }
    }
}
