use inkwell::basic_block::BasicBlock;

use crate::ast::IfExpr;
use crate::semantic::SemanticAnalysis;

use super::super::{CodegenValue, CodeGenerator, ValueKind};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_if(
        &mut self,
        if_expr: &IfExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        self.lower_if_chain(
            &if_expr.condition,
            &if_expr.then_branch,
            &if_expr.elif_branches,
            &if_expr.else_branch,
            analysis,
        )
    }

    fn lower_if_chain(
        &mut self,
        condition: &crate::ast::Expr,
        then_branch: &crate::ast::Expr,
        elif_branches: &[(crate::ast::Expr, crate::ast::Expr)],
        else_branch: &crate::ast::Expr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let (current_cond, current_then, tail) = if let Some((head, rest)) = elif_branches.split_first() {
            (condition, then_branch, Some((head, rest)))
        } else {
            (condition, then_branch, None)
        };

        let else_expr = if let Some(((elif_cond, elif_then), rest)) = tail {
            let nested = crate::ast::IfExpr {
                condition: Box::new(elif_cond.clone()),
                then_branch: Box::new(elif_then.clone()),
                elif_branches: rest.to_vec(),
                else_branch: Box::new(else_branch.clone()),
            };
            return self.lower_if(&nested, analysis);
        } else {
            else_branch
        };

        let cond_value = self.lower_expr(current_cond, analysis)?.into_bool()?;
        let current_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| "No hay bloque de insercion activo".to_string())?;
        let function = current_block
            .get_parent()
            .ok_or_else(|| "No hay funcion activa".to_string())?;

        let then_block = self.context.append_basic_block(function, "if_then");
        let else_block = self.context.append_basic_block(function, "if_else");
        let merge_block = self.context.append_basic_block(function, "if_merge");

        self.builder
            .build_conditional_branch(cond_value, then_block, else_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(then_block);
        let then_value = self.lower_expr(current_then, analysis)?;
        self.builder
            .build_unconditional_branch(merge_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(else_block);
        let else_value = self.lower_expr(else_expr, analysis)?;
        self.builder
            .build_unconditional_branch(merge_block)
            .map_err(|e| e.to_string())?;

        if then_value.kind() != else_value.kind() {
            return Err("Tipos incompatibles entre ramas if/else".to_string());
        }

        self.builder.position_at_end(merge_block);
        self.build_phi_value(then_value.kind(), then_block, else_block, then_value, else_value)
    }

    fn build_phi_value(
        &mut self,
        kind: ValueKind,
        then_block: BasicBlock<'ctx>,
        else_block: BasicBlock<'ctx>,
        then_value: CodegenValue<'ctx>,
        else_value: CodegenValue<'ctx>,
    ) -> Result<CodegenValue<'ctx>, String> {
        match kind {
            ValueKind::Number => {
                let phi = self
                    .builder
                    .build_phi(self.f64_type, "iftmp")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&then_value.into_number()?, then_block),
                    (&else_value.into_number()?, else_block),
                ]);
                Ok(CodegenValue::Number(phi.as_basic_value().into_float_value()))
            }
            ValueKind::Bool => {
                let phi = self
                    .builder
                    .build_phi(self.bool_type, "iftmp_bool")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&then_value.into_bool()?, then_block),
                    (&else_value.into_bool()?, else_block),
                ]);
                Ok(CodegenValue::Bool(phi.as_basic_value().into_int_value()))
            }
        }
    }
}
