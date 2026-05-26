use crate::ast::{Expr, IfExpr, KindExpr, LiteralValue, Pattern, Span};
use crate::semantic::types::SemanticType;

use super::SemanticAnalyzer;

impl SemanticAnalyzer {
    /// Determina si una expresion garantiza un valor en todos sus caminos.
    pub(super) fn guarantees_value(&self, expr: &Expr) -> bool {
        match &expr.kind {
            KindExpr::While(while_expr) => self.guarantees_value(&while_expr.body),
            KindExpr::For(for_expr) => self.guarantees_value(&for_expr.body),
            KindExpr::Block(block) => block
                .expressions
                .last()
                .map(|last| self.guarantees_value(last))
                .unwrap_or(false),
            KindExpr::If(if_expr) => {
                self.guarantees_value(&if_expr.then_branch)
                    && if_expr
                        .elif_branches
                        .iter()
                        .all(|(_, body)| self.guarantees_value(body))
                    && self.guarantees_value(&if_expr.else_branch)
            }
            KindExpr::Let(let_expr) => self.guarantees_value(&let_expr.body),
            KindExpr::Match(match_expr) => {
                !match_expr.cases.is_empty()
                    && match_expr
                        .cases
                        .iter()
                        .all(|case| self.guarantees_value(&case.body))
            }
            _ => true,
        }
    }

    /// Detecta si una expresion es seguramente no terminante.
    pub(super) fn definitely_non_terminating(&self, expr: &Expr) -> bool {
        match &expr.kind {
            KindExpr::While(while_expr) => {
                matches!(self.eval_const_bool(&while_expr.condition), Some(true))
            }
            KindExpr::Block(block) => {
                for sub in &block.expressions {
                    if self.definitely_non_terminating(sub) {
                        return true;
                    }
                }
                false
            }
            KindExpr::If(if_expr) => {
                if let Some(cond) = self.eval_const_bool(&if_expr.condition) {
                    if cond {
                        return self.definitely_non_terminating(&if_expr.then_branch);
                    }

                    for (elif_cond, elif_body) in &if_expr.elif_branches {
                        match self.eval_const_bool(elif_cond) {
                            Some(false) => continue,
                            Some(true) => return self.definitely_non_terminating(elif_body),
                            None => return false,
                        }
                    }

                    return self.definitely_non_terminating(&if_expr.else_branch);
                }

                self.definitely_non_terminating(&if_expr.then_branch)
                    && if_expr
                        .elif_branches
                        .iter()
                        .all(|(_, body)| self.definitely_non_terminating(body))
                    && self.definitely_non_terminating(&if_expr.else_branch)
            }
            KindExpr::Match(match_expr) => {
                if match_expr.cases.is_empty() {
                    return false;
                }

                let all_non_terminating = match_expr
                    .cases
                    .iter()
                    .all(|case| self.definitely_non_terminating(&case.body));
                if !all_non_terminating {
                    return false;
                }

                if match_expr
                    .cases
                    .iter()
                    .any(|case| matches!(case.pattern, Pattern::Default))
                {
                    return true;
                }

                let mut saw_true = false;
                let mut saw_false = false;
                for case in &match_expr.cases {
                    match case.pattern {
                        Pattern::Literal(LiteralValue::Bool(true)) => saw_true = true,
                        Pattern::Literal(LiteralValue::Bool(false)) => saw_false = true,
                        _ => {}
                    }
                }

                saw_true && saw_false
            }
            _ => false,
        }
    }

    /// Indica el minimo de iteraciones garantizadas por un `while`.
    pub(super) fn while_min_iterations(&self, condition: &Expr) -> Option<usize> {
        self.eval_const_bool(condition)
            .map(|cond| if cond { 1 } else { 0 })
    }

    /// Indica el minimo de iteraciones garantizadas por una iteracion sobre un iterable.
    pub(super) fn iterable_min_iterations(&self, iterable: &Expr) -> Option<usize> {
        match &iterable.kind {
            KindExpr::Array(array) => Some(if array.elements.is_empty() { 0 } else { 1 }),
            KindExpr::Call(call) => {
                let KindExpr::Variable(var) = &call.callee.kind else {
                    return None;
                };

                if var.name != "range" || call.arguments.len() != 2 {
                    return None;
                }

                let start = self.eval_const_number(&call.arguments[0])?;
                let end = self.eval_const_number(&call.arguments[1])?;
                Some(if end > start { 1 } else { 0 })
            }
            _ => None,
        }
    }

    /// Evalua una expresion como numero constante cuando es posible.
    pub(super) fn eval_const_number(&self, expr: &Expr) -> Option<f64> {
        use crate::ast::BinaryOperator;
        use crate::ast::UnaryOperator;

        match &expr.kind {
            KindExpr::Literal(lit) => match lit.value {
                LiteralValue::Number(n) => Some(n),
                _ => None,
            },
            KindExpr::Unary(unary) => {
                if matches!(unary.operator, UnaryOperator::Negate) {
                    self.eval_const_number(&unary.right).map(|value| -value)
                } else {
                    None
                }
            }
            KindExpr::Binary(bin) => {
                let left = self.eval_const_number(&bin.left)?;
                let right = self.eval_const_number(&bin.right)?;

                match bin.operator {
                    BinaryOperator::Add => Some(left + right),
                    BinaryOperator::Sub => Some(left - right),
                    BinaryOperator::Mul => Some(left * right),
                    BinaryOperator::Div => Some(left / right),
                    BinaryOperator::Pow => Some(left.powf(right)),
                    BinaryOperator::Mod => Some(left % right),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Evalua una expresion como booleano constante cuando es posible.
    pub(super) fn eval_const_bool(&self, expr: &Expr) -> Option<bool> {
        use crate::ast::BinaryOperator;
        use crate::ast::UnaryOperator;

        match &expr.kind {
            KindExpr::Literal(lit) => match lit.value {
                LiteralValue::Bool(b) => Some(b),
                _ => None,
            },
            KindExpr::Unary(unary) => {
                if matches!(unary.operator, UnaryOperator::Not) {
                    self.eval_const_bool(&unary.right).map(|value| !value)
                } else {
                    None
                }
            }
            KindExpr::Binary(bin) => match bin.operator {
                BinaryOperator::And => {
                    Some(self.eval_const_bool(&bin.left)? && self.eval_const_bool(&bin.right)?)
                }
                BinaryOperator::Or => {
                    Some(self.eval_const_bool(&bin.left)? || self.eval_const_bool(&bin.right)?)
                }
                BinaryOperator::Equal => {
                    if let (Some(left), Some(right)) = (
                        self.eval_const_number(&bin.left),
                        self.eval_const_number(&bin.right),
                    ) {
                        Some(left.to_bits() == right.to_bits())
                    } else if let (Some(left), Some(right)) = (
                        self.eval_const_bool(&bin.left),
                        self.eval_const_bool(&bin.right),
                    ) {
                        Some(left == right)
                    } else {
                        let left = self.eval_const_literal(&bin.left)?;
                        let right = self.eval_const_literal(&bin.right)?;
                        Some(self.literal_values_equal(&left, &right))
                    }
                }
                BinaryOperator::NotEqual => {
                    if let (Some(left), Some(right)) = (
                        self.eval_const_number(&bin.left),
                        self.eval_const_number(&bin.right),
                    ) {
                        Some(left.to_bits() != right.to_bits())
                    } else if let (Some(left), Some(right)) = (
                        self.eval_const_bool(&bin.left),
                        self.eval_const_bool(&bin.right),
                    ) {
                        Some(left != right)
                    } else {
                        let left = self.eval_const_literal(&bin.left)?;
                        let right = self.eval_const_literal(&bin.right)?;
                        Some(!self.literal_values_equal(&left, &right))
                    }
                }
                BinaryOperator::Less => {
                    Some(self.eval_const_number(&bin.left)? < self.eval_const_number(&bin.right)?)
                }
                BinaryOperator::Greater => {
                    Some(self.eval_const_number(&bin.left)? > self.eval_const_number(&bin.right)?)
                }
                BinaryOperator::LessEqual => {
                    Some(self.eval_const_number(&bin.left)? <= self.eval_const_number(&bin.right)?)
                }
                BinaryOperator::GreaterEqual => {
                    Some(self.eval_const_number(&bin.left)? >= self.eval_const_number(&bin.right)?)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Recupera un literal constante sin perder su valor original.
    pub(super) fn eval_const_literal(&self, expr: &Expr) -> Option<LiteralValue> {
        let KindExpr::Literal(lit) = &expr.kind else {
            return None;
        };

        match &lit.value {
            LiteralValue::Number(n) => Some(LiteralValue::Number(*n)),
            LiteralValue::String(s) => Some(LiteralValue::String(s.clone())),
            LiteralValue::Bool(b) => Some(LiteralValue::Bool(*b)),
        }
    }

    /// Compara dos literales respetando su tipo y representacion.
    pub(super) fn literal_values_equal(&self, left: &LiteralValue, right: &LiteralValue) -> bool {
        match (left, right) {
            (LiteralValue::Number(a), LiteralValue::Number(b)) => a.to_bits() == b.to_bits(),
            (LiteralValue::String(a), LiteralValue::String(b)) => a == b,
            (LiteralValue::Bool(a), LiteralValue::Bool(b)) => a == b,
            _ => false,
        }
    }

    /// Reporta ramas inalcanzables en una expresion condicional.
    pub(super) fn report_unreachable_if_branches(&mut self, if_expr: &IfExpr, span: Span) {
        match self.eval_const_bool(&if_expr.condition) {
            Some(true) => {
                for (_, elif_body) in &if_expr.elif_branches {
                    self.diagnostics.error(
                        "Rama elif inalcanzable: la condicion del if es siempre true",
                        elif_body.span,
                    );
                }
                self.diagnostics.error(
                    "Rama else inalcanzable: la condicion del if es siempre true",
                    if_expr.else_branch.span,
                );
                return;
            }
            Some(false) => {
                self.diagnostics.error(
                    "Rama then inalcanzable: la condicion del if es siempre false",
                    if_expr.then_branch.span,
                );
            }
            None => {}
        }

        let mut branch_already_taken = false;
        for (elif_cond, elif_body) in &if_expr.elif_branches {
            if branch_already_taken {
                self.diagnostics.error(
                    "Rama elif inalcanzable: hay una rama previa siempre verdadera",
                    elif_body.span,
                );
                continue;
            }

            if let Some(true) = self.eval_const_bool(elif_cond) {
                branch_already_taken = true;
            }
        }

        if branch_already_taken {
            self.diagnostics.error(
                "Rama else inalcanzable: hay una rama previa siempre verdadera",
                if_expr.else_branch.span,
            );
        }

        let _ = span;
    }

    /// Recolecta accesos a `self.campo` dentro de una expresion.
    pub(super) fn collect_self_member_accesses(&self, expr: &Expr, out: &mut Vec<String>) {
        use KindExpr::*;
        match &expr.kind {
            MemberAccess(member) => {
                if let KindExpr::Variable(var) = &member.object.kind {
                    if var.name == "self" {
                        out.push(member.field.clone());
                        self.collect_self_member_accesses(&member.object, out);
                        return;
                    }
                }
                self.collect_self_member_accesses(&member.object, out);
            }
            Binary(b) => {
                self.collect_self_member_accesses(&b.left, out);
                self.collect_self_member_accesses(&b.right, out);
            }
            Unary(u) => {
                self.collect_self_member_accesses(&u.right, out);
            }
            Call(c) => {
                self.collect_self_member_accesses(&c.callee, out);
                for a in &c.arguments {
                    self.collect_self_member_accesses(a, out);
                }
            }
            MacroCall(m) => {
                for a in &m.arguments {
                    self.collect_self_member_accesses(&a.value, out);
                }
                if let Some(action) = &m.action {
                    self.collect_self_member_accesses(action, out);
                }
            }
            Let(l) => {
                for b in &l.bindings {
                    self.collect_self_member_accesses(&b.initializer, out);
                }
                self.collect_self_member_accesses(&l.body, out);
            }
            Block(b) => {
                for e in &b.expressions {
                    self.collect_self_member_accesses(e, out);
                }
            }
            If(i) => {
                self.collect_self_member_accesses(&i.condition, out);
                self.collect_self_member_accesses(&i.then_branch, out);
                for (cond, body) in &i.elif_branches {
                    self.collect_self_member_accesses(cond, out);
                    self.collect_self_member_accesses(body, out);
                }
                self.collect_self_member_accesses(&i.else_branch, out);
            }
            While(w) => {
                self.collect_self_member_accesses(&w.condition, out);
                self.collect_self_member_accesses(&w.body, out);
            }
            For(f) => {
                self.collect_self_member_accesses(&f.iterable, out);
                self.collect_self_member_accesses(&f.body, out);
            }
            Assign(a) => {
                self.collect_self_member_accesses(&a.target, out);
                self.collect_self_member_accesses(&a.value, out);
            }
            Index(ix) => {
                self.collect_self_member_accesses(&ix.object, out);
                self.collect_self_member_accesses(&ix.index, out);
            }
            Array(arr) => {
                for e in &arr.elements {
                    self.collect_self_member_accesses(e, out);
                }
            }
            ArrayComprehension(ac) => {
                self.collect_self_member_accesses(&ac.iterable, out);
                self.collect_self_member_accesses(&ac.element, out);
            }
            Lambda(lam) => {
                self.collect_self_member_accesses(&lam.body, out);
            }
            New(n) => {
                for a in &n.arguments {
                    self.collect_self_member_accesses(a, out);
                }
            }
            Is(is_e) => {
                self.collect_self_member_accesses(&is_e.expression, out);
            }
            As(as_e) => {
                self.collect_self_member_accesses(&as_e.expression, out);
            }
            Match(m) => {
                self.collect_self_member_accesses(&m.expression, out);
                for c in &m.cases {
                    self.collect_self_member_accesses(&c.body, out);
                }
            }
            Literal(_) | Variable(_) | BaseCall(_) => {}
        }
    }

    /// Comprueba si un patron coincide con un literal ya conocido.
    pub(super) fn pattern_matches_known_literal(
        &mut self,
        pattern: &Pattern,
        literal: &LiteralValue,
        span: Span,
    ) -> Option<bool> {
        match pattern {
            Pattern::Literal(pattern_lit) => Some(self.literal_values_equal(pattern_lit, literal)),
            Pattern::Default => Some(true),
            Pattern::Identifier {
                type_restriction: None,
                ..
            } => Some(true),
            Pattern::Identifier {
                type_restriction: Some(type_ref),
                ..
            } => {
                let restricted_ty = self.resolve_type_ref(Some(type_ref), span);
                let literal_ty = self.literal_type(literal);
                Some(
                    self.is_compatible_type(&restricted_ty, &literal_ty)
                        || self.is_compatible_type(&literal_ty, &restricted_ty),
                )
            }
            _ => None,
        }
    }

    /// Devuelve el tipo de cobertura esperable para un patron.
    pub(super) fn pattern_coverage_type(
        &mut self,
        pattern: &Pattern,
        span: Span,
    ) -> Option<SemanticType> {
        match pattern {
            Pattern::Identifier {
                type_restriction: Some(type_ref),
                ..
            } => Some(self.resolve_type_ref(Some(type_ref), span)),
            _ => None,
        }
    }

    /// Deriva el tipo semantico de un literal.
    pub(super) fn literal_type(&self, literal: &LiteralValue) -> SemanticType {
        match literal {
            LiteralValue::Number(_) => SemanticType::Number,
            LiteralValue::String(_) => SemanticType::String,
            LiteralValue::Bool(_) => SemanticType::Boolean,
        }
    }

    /// Genera una llave estable para identificar un patron literal.
    pub(super) fn literal_pattern_key(literal: &LiteralValue) -> String {
        match literal {
            LiteralValue::Number(n) => n.to_string(),
            LiteralValue::String(s) => format!("\"{}\"", s),
            LiteralValue::Bool(b) => b.to_string(),
        }
    }
}
