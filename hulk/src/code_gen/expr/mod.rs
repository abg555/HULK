mod binary;
mod block;
mod call;
mod if_else;
mod lambda;
mod let_assign;
mod literals;
mod loops;
mod macros;
mod match_expr;
mod objects;
mod arrays;
mod unary;

use crate::ast::{Expr, KindExpr};
use crate::semantic::SemanticAnalysis;

use super::{CodeGenerator, CodegenValue};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_expr(
        &mut self,
        expr: &Expr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        match &expr.kind {
            KindExpr::Literal(literal) => self.lower_literal(literal),
            KindExpr::Unary(unary) => self.lower_unary(unary, analysis),
            KindExpr::Binary(binary) => {
                let left = self.lower_expr(&binary.left, analysis)?;
                let right = self.lower_expr(&binary.right, analysis)?;

                self.build_binary_op(&binary.operator, left, right)
            }
            KindExpr::Let(let_expr) => self.lower_let(let_expr, analysis),
            KindExpr::Variable(variable) => self.lower_variable(variable),
            KindExpr::Assign(assign) => self.lower_assign(assign, analysis),
            KindExpr::Block(block) => self.lower_block(block, analysis),
            KindExpr::Call(call) => self.lower_call(call, analysis),
            KindExpr::MacroCall(call) => self.lower_macro_call(call, analysis),
            KindExpr::BaseCall(call) => self.lower_base_call(call, analysis),
            KindExpr::Lambda(lambda) => self.lower_lambda(lambda, analysis),
            KindExpr::Match(match_expr) => self.lower_match(match_expr, analysis),
            KindExpr::New(new_expr) => self.lower_new(new_expr, analysis),
            KindExpr::MemberAccess(member) => self.lower_member_access(member, analysis),
            KindExpr::If(if_expr) => self.lower_if(if_expr, analysis),
            KindExpr::While(while_expr) => self.lower_while(while_expr, analysis),
            KindExpr::For(for_expr) => self.lower_for(for_expr, analysis),
            KindExpr::Is(is_expr) => self.lower_is(is_expr, analysis),
            KindExpr::As(as_expr) => self.lower_as(as_expr, analysis),
            KindExpr::Array(array_expr) => self.lower_array(expr, array_expr, analysis),
            KindExpr::Index(index_expr) => self.lower_index(index_expr, analysis),
            KindExpr::ArrayComprehension(comp) => self.lower_array_comprehension(comp, analysis),
        }
    }
}
