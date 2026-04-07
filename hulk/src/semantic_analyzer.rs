use std::collections::HashMap;

use crate::ast::*;
use crate::diagnostics::{Diagnostic, DiagnosticCollector};
use crate::symbol_table::{Symbol, SymbolKind, SymbolTable};
use crate::types::SemanticType;

#[derive(Debug)]
pub struct SemanticAnalysis {
    pub inferred_types: HashMap<NodeId, SemanticType>,
}

pub struct SemanticAnalyzer {
    symbols: SymbolTable,
    diagnostics: DiagnosticCollector,
    inferred_types: HashMap<NodeId, SemanticType>,
    current_return_type: Option<SemanticType>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            diagnostics: DiagnosticCollector::new(),
            inferred_types: HashMap::new(),
            current_return_type: None,
        }
    }

    pub fn analyze(mut self, program: &Program) -> Result<SemanticAnalysis, Vec<Diagnostic>> {
        self.collect_top_level(program);

        for item in &program.items {
            self.check_item(item);
        }

        if self.diagnostics.has_errors() {
            Err(self.diagnostics.into_vec())
        } else {
            Ok(SemanticAnalysis {
                inferred_types: self.inferred_types,
            })
        }
    }

    fn collect_top_level(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    let params = func
                        .params
                        .iter()
                        .map(|p| self.resolve_type_ref(p.types.as_ref()))
                        .collect::<Vec<_>>();
                    let ret = func
                        .return_type
                        .as_ref()
                        .map(SemanticType::from_type_ref)
                        .unwrap_or(SemanticType::Unknown);
                    self.define_symbol(
                        &func.name,
                        SymbolKind::Function,
                        SemanticType::Function(params, Box::new(ret)),
                    );
                }
                Item::Type(typ) => {
                    self.define_symbol(&typ.name, SymbolKind::Type, SemanticType::Custom(typ.name.clone()));
                }
                Item::Protocol(proto) => {
                    self.define_symbol(
                        &proto.name,
                        SymbolKind::Protocol,
                        SemanticType::Custom(proto.name.clone()),
                    );
                }
                Item::Macro(macr) => {
                    self.define_symbol(&macr.name, SymbolKind::Macro, SemanticType::Unknown);
                }
                Item::GlobalExpr(_) => {}
            }
        }
    }

    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Function(func) => self.check_function_decl(func),
            Item::Type(typ) => self.check_type_decl(typ),
            Item::Protocol(proto) => self.check_protocol_decl(proto),
            Item::Macro(macr) => self.check_macro_decl(macr),
            Item::GlobalExpr(expr) => {
                self.check_expr(expr);
            }
        }
    }

    fn check_function_decl(&mut self, func: &FunctionDecl) {
        self.symbols.enter_scope();
        for param in &func.params {
            let typ = self.resolve_type_ref(param.types.as_ref());
            self.define_local(&param.name, SymbolKind::Variable, typ, func.body.span);
        }

        let prev_return = self.current_return_type.clone();
        self.current_return_type = func.return_type.as_ref().map(SemanticType::from_type_ref);

        let body_ty = self.check_expr(&func.body);
        if let Some(expected) = &self.current_return_type
            && !expected.is_assignable_from(&body_ty)
        {
            self.diagnostics.error(
                format!(
                    "La funcion {} retorna {}, se esperaba {}",
                    func.name, body_ty, expected
                ),
                func.body.span,
            );
        }

        self.current_return_type = prev_return;
        self.symbols.exit_scope();
    }

    fn check_type_decl(&mut self, typ: &TypeDecl) {
        if let Some(parent) = &typ.parent {
            let parent_type = self.resolve_type_ref(Some(parent));
            if !matches!(parent_type, SemanticType::Custom(_)) {
                self.diagnostics.error(
                    format!("El padre de {} debe ser un tipo nombrado", typ.name),
                    Span { start: 0, end: 0 },
                );
            }
        }

        self.symbols.enter_scope();

        for field in &typ.fields {
            let init_ty = self.check_expr(&field.initializer);
            let declared = self.resolve_type_ref(field.type_annotation.as_ref());
            if !declared.is_assignable_from(&init_ty) {
                self.diagnostics.error(
                    format!(
                        "El campo {} espera {}, pero recibe {}",
                        field.name, declared, init_ty
                    ),
                    field.initializer.span,
                );
            }
            self.define_local(
                &field.name,
                SymbolKind::Variable,
                declared,
                field.initializer.span,
            );
        }

        for method in &typ.methods {
            self.check_function_decl(method);
        }

        self.symbols.exit_scope();
    }

    fn check_protocol_decl(&mut self, proto: &ProtocolDecl) {
        if let Some(parent) = &proto.parent {
            let parent_type = self.resolve_type_ref(Some(parent.as_ref()));
            if !matches!(parent_type, SemanticType::Custom(_)) {
                self.diagnostics.error(
                    format!("El protocolo {} debe extender otro protocolo por nombre", proto.name),
                    Span { start: 0, end: 0 },
                );
            }
        }
    }

    fn check_macro_decl(&mut self, macr: &MacroDecl) {
        self.symbols.enter_scope();
        for param in &macr.params {
            let typ = self.resolve_type_ref(param.type_info.as_ref());
            self.define_local(&param.name, SymbolKind::Variable, typ, macr.body.span);
        }
        self.check_expr(&macr.body);
        self.symbols.exit_scope();
    }

    fn check_expr(&mut self, expr: &Expr) -> SemanticType {
        let inferred = match &expr.kind {
            KindExpr::Literal(lit) => match lit.value {
                LiteralValue::Number(_) => SemanticType::Number,
                LiteralValue::String(_) => SemanticType::String,
                LiteralValue::Bool(_) => SemanticType::Boolean,
            },
            KindExpr::Variable(var) => {
                if let Some(symbol) = self.symbols.lookup(&var.name) {
                    symbol.typ.clone()
                } else {
                    self.diagnostics.error(
                        format!("Identificador no definido: {}", var.name),
                        expr.span,
                    );
                    SemanticType::Unknown
                }
            }
            KindExpr::Binary(bin) => {
                let left = self.check_expr(&bin.left);
                let right = self.check_expr(&bin.right);
                self.check_binary(expr.span, &bin.operator, left, right)
            }
            KindExpr::Unary(unary) => {
                let right = self.check_expr(&unary.right);
                match unary.operator {
                    UnaryOperator::Negate => {
                        self.expect_type(expr.span, &right, &SemanticType::Number, "operador '-' ");
                        SemanticType::Number
                    }
                    UnaryOperator::Not => {
                        self.expect_type(expr.span, &right, &SemanticType::Boolean, "operador '!' ");
                        SemanticType::Boolean
                    }
                }
            }
            KindExpr::Call(call) => {
                let callee = self.check_expr(&call.callee);
                let arg_types = call
                    .arguments
                    .iter()
                    .map(|arg| self.check_expr(arg))
                    .collect::<Vec<_>>();

                if let SemanticType::Function(params, ret) = callee {
                    if params.len() != arg_types.len() {
                        self.diagnostics.error(
                            format!(
                                "Aridad invalida: se esperaban {} argumentos y llegaron {}",
                                params.len(),
                                arg_types.len()
                            ),
                            expr.span,
                        );
                    }

                    for (idx, (expected, actual)) in params.iter().zip(arg_types.iter()).enumerate() {
                        if !expected.is_assignable_from(actual) {
                            self.diagnostics.error(
                                format!(
                                    "Argumento {} incompatible: se esperaba {}, se obtuvo {}",
                                    idx + 1,
                                    expected,
                                    actual
                                ),
                                expr.span,
                            );
                        }
                    }

                    *ret
                } else {
                    self.diagnostics.error("Intento de llamada sobre un valor no invocable", expr.span);
                    SemanticType::Unknown
                }
            }
            KindExpr::BaseCall(_) => SemanticType::Unknown,
            KindExpr::MacroCall(call) => {
                if self.symbols.lookup(&call.name).is_none() {
                    self.diagnostics.error(
                        format!("Macro no definida: {}", call.name),
                        expr.span,
                    );
                }
                for arg in &call.arguments {
                    self.check_expr(&arg.value);
                }
                if let Some(action) = &call.action {
                    self.check_expr(action);
                }
                SemanticType::Unknown
            }
            KindExpr::Let(let_expr) => {
                self.symbols.enter_scope();
                for binding in &let_expr.bindings {
                    let init_ty = self.check_expr(&binding.initializer);
                    let declared = self.resolve_type_ref(binding.types.as_ref());
                    if !declared.is_assignable_from(&init_ty) {
                        self.diagnostics.error(
                            format!(
                                "Binding {} incompatible: se esperaba {}, se obtuvo {}",
                                binding.name, declared, init_ty
                            ),
                            binding.initializer.span,
                        );
                    }
                    let stored = if matches!(declared, SemanticType::Unknown) {
                        init_ty
                    } else {
                        declared
                    };
                    self.define_local(
                        &binding.name,
                        SymbolKind::Variable,
                        stored,
                        binding.initializer.span,
                    );
                }
                let body_ty = self.check_expr(&let_expr.body);
                self.symbols.exit_scope();
                body_ty
            }
            KindExpr::Block(block) => {
                self.symbols.enter_scope();
                let mut last = SemanticType::Unknown;
                for sub in &block.expressions {
                    last = self.check_expr(sub);
                }
                self.symbols.exit_scope();
                last
            }
            KindExpr::If(if_expr) => {
                let cond_ty = self.check_expr(&if_expr.condition);
                self.expect_type(expr.span, &cond_ty, &SemanticType::Boolean, "condicion de if");

                let then_ty = self.check_expr(&if_expr.then_branch);
                for (elif_cond, elif_body) in &if_expr.elif_branches {
                    let elif_cond_ty = self.check_expr(elif_cond);
                    self.expect_type(expr.span, &elif_cond_ty, &SemanticType::Boolean, "condicion de elif");
                    let _ = self.check_expr(elif_body);
                }

                let else_ty = self.check_expr(&if_expr.else_branch);
                if then_ty.is_assignable_from(&else_ty) {
                    then_ty
                } else if else_ty.is_assignable_from(&then_ty) {
                    else_ty
                } else {
                    SemanticType::Unknown
                }
            }
            KindExpr::While(while_expr) => {
                let cond_ty = self.check_expr(&while_expr.condition);
                self.expect_type(expr.span, &cond_ty, &SemanticType::Boolean, "condicion de while");
                self.check_expr(&while_expr.body);
                SemanticType::Unknown
            }
            KindExpr::For(for_expr) => {
                let iterable_ty = self.check_expr(&for_expr.iterable);
                self.symbols.enter_scope();
                let element_ty = match iterable_ty {
                    SemanticType::Vector(inner) => *inner,
                    other => {
                        self.diagnostics.error(
                            format!("El for requiere iterable vectorial, se obtuvo {}", other),
                            expr.span,
                        );
                        SemanticType::Unknown
                    }
                };
                self.define_local(&for_expr.variable, SymbolKind::Variable, element_ty, expr.span);
                self.check_expr(&for_expr.body);
                self.symbols.exit_scope();
                SemanticType::Unknown
            }
            KindExpr::Assign(assign) => {
                let target_ty = self.check_expr(&assign.target);
                let value_ty = self.check_expr(&assign.value);
                if !target_ty.is_assignable_from(&value_ty) {
                    self.diagnostics.error(
                        format!(
                            "Asignacion incompatible: se esperaba {}, se obtuvo {}",
                            target_ty, value_ty
                        ),
                        expr.span,
                    );
                }
                target_ty
            }
            KindExpr::MemberAccess(member) => {
                self.check_expr(&member.object);
                SemanticType::Unknown
            }
            KindExpr::Index(index) => {
                let obj_ty = self.check_expr(&index.object);
                let idx_ty = self.check_expr(&index.index);
                self.expect_type(expr.span, &idx_ty, &SemanticType::Number, "indice de arreglo");
                match obj_ty {
                    SemanticType::Vector(inner) => *inner,
                    other => {
                        self.diagnostics.error(
                            format!("El operador [] requiere vector, se obtuvo {}", other),
                            expr.span,
                        );
                        SemanticType::Unknown
                    }
                }
            }
            KindExpr::Array(array) => {
                if array.elements.is_empty() {
                    SemanticType::Vector(Box::new(SemanticType::Unknown))
                } else {
                    let mut element_ty = self.check_expr(&array.elements[0]);
                    for elem in array.elements.iter().skip(1) {
                        let current = self.check_expr(elem);
                        if !element_ty.is_assignable_from(&current)
                            && !current.is_assignable_from(&element_ty)
                        {
                            element_ty = SemanticType::Unknown;
                        }
                    }
                    SemanticType::Vector(Box::new(element_ty))
                }
            }
            KindExpr::ArrayComprehension(comp) => {
                let iter_ty = self.check_expr(&comp.iterable);
                self.symbols.enter_scope();
                let loop_ty = match iter_ty {
                    SemanticType::Vector(inner) => *inner,
                    _ => SemanticType::Unknown,
                };
                self.define_local(&comp.variable, SymbolKind::Variable, loop_ty, expr.span);
                let elem_ty = self.check_expr(&comp.element);
                self.symbols.exit_scope();
                SemanticType::Vector(Box::new(elem_ty))
            }
            KindExpr::Lambda(lambda) => {
                self.symbols.enter_scope();
                let mut params = Vec::new();
                for param in &lambda.params {
                    let param_ty = self.resolve_type_ref(param.types.as_ref());
                    self.define_local(&param.name, SymbolKind::Variable, param_ty.clone(), expr.span);
                    params.push(param_ty);
                }
                let body_ty = self.check_expr(&lambda.body);
                self.symbols.exit_scope();
                let ret = lambda
                    .return_type
                    .as_ref()
                    .map(SemanticType::from_type_ref)
                    .unwrap_or(body_ty);
                SemanticType::Function(params, Box::new(ret))
            }
            KindExpr::New(new_expr) => {
                if let Some(symbol) = self.symbols.lookup(&new_expr.type_name) {
                    if symbol.kind != SymbolKind::Type {
                        self.diagnostics.error(
                            format!("{} no es un tipo construible", new_expr.type_name),
                            expr.span,
                        );
                    }
                } else {
                    self.diagnostics.error(
                        format!("Tipo no definido: {}", new_expr.type_name),
                        expr.span,
                    );
                }
                for arg in &new_expr.arguments {
                    self.check_expr(arg);
                }
                SemanticType::Custom(new_expr.type_name.clone())
            }
            KindExpr::Is(is_expr) => {
                self.check_expr(&is_expr.expression);
                let _ = self.resolve_type_ref(Some(&is_expr.type_info));
                SemanticType::Boolean
            }
            KindExpr::As(as_expr) => {
                self.check_expr(&as_expr.expression);
                self.resolve_type_ref(Some(&as_expr.type_info))
            }
            KindExpr::Match(match_expr) => {
                self.check_expr(&match_expr.expression);
                let mut merged = SemanticType::Unknown;
                for case in &match_expr.cases {
                    self.check_pattern(&case.pattern);
                    let case_ty = self.check_expr(&case.body);
                    if matches!(merged, SemanticType::Unknown) {
                        merged = case_ty;
                    }
                }
                merged
            }
        };

        self.inferred_types.insert(expr.id, inferred.clone());
        inferred
    }

    fn check_pattern(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Identifier {
                name,
                type_restriction,
            } => {
                let typ = self.resolve_type_ref(type_restriction.as_ref());
                self.define_local(name, SymbolKind::Variable, typ, Span { start: 0, end: 0 });
            }
            Pattern::Binary { left, right, .. } => {
                self.check_pattern(left);
                self.check_pattern(right);
            }
            Pattern::Unary { operand, .. } => self.check_pattern(operand),
            Pattern::Literal(_) | Pattern::Default => {}
        }
    }

    fn check_binary(
        &mut self,
        span: Span,
        operator: &BinaryOperator,
        left: SemanticType,
        right: SemanticType,
    ) -> SemanticType {
        match operator {
            BinaryOperator::Add
            | BinaryOperator::Sub
            | BinaryOperator::Mul
            | BinaryOperator::Div
            | BinaryOperator::Pow
            | BinaryOperator::Mod => {
                self.expect_type(span, &left, &SemanticType::Number, "operador aritmetico");
                self.expect_type(span, &right, &SemanticType::Number, "operador aritmetico");
                SemanticType::Number
            }
            BinaryOperator::And | BinaryOperator::Or => {
                self.expect_type(span, &left, &SemanticType::Boolean, "operador logico");
                self.expect_type(span, &right, &SemanticType::Boolean, "operador logico");
                SemanticType::Boolean
            }
            BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::Greater
            | BinaryOperator::LessEqual
            | BinaryOperator::GreaterEqual => {
                if !left.is_assignable_from(&right) && !right.is_assignable_from(&left) {
                    self.diagnostics.error(
                        format!("Comparacion incompatible entre {} y {}", left, right),
                        span,
                    );
                }
                SemanticType::Boolean
            }
            BinaryOperator::Concat | BinaryOperator::FullConcat => SemanticType::String,
        }
    }

    fn resolve_type_ref(&mut self, type_ref: Option<&TypeRef>) -> SemanticType {
        let Some(type_ref) = type_ref else {
            return SemanticType::Unknown;
        };

        let resolved = SemanticType::from_type_ref(type_ref);
        if let SemanticType::Custom(name) = &resolved
            && self.symbols.lookup(name).is_none()
        {
            self.diagnostics.error(
                format!("Tipo no definido: {}", name),
                Span { start: 0, end: 0 },
            );
        }

        resolved
    }

    fn expect_type(&mut self, span: Span, actual: &SemanticType, expected: &SemanticType, ctx: &str) {
        if !expected.is_assignable_from(actual) {
            self.diagnostics.error(
                format!("Tipo incompatible en {}: se esperaba {}, se obtuvo {}", ctx, expected, actual),
                span,
            );
        }
    }

    fn define_symbol(&mut self, name: &str, kind: SymbolKind, typ: SemanticType) {
        let symbol = Symbol {
            name: name.to_string(),
            kind,
            typ,
        };

        if !self.symbols.define(symbol) {
            self.diagnostics.error(
                format!("Redefinicion de simbolo top-level: {}", name),
                Span { start: 0, end: 0 },
            );
        }
    }

    fn define_local(&mut self, name: &str, kind: SymbolKind, typ: SemanticType, span: Span) {
        let symbol = Symbol {
            name: name.to_string(),
            kind,
            typ,
        };

        if !self.symbols.define(symbol) {
            self.diagnostics.error(
                format!("Redefinicion de simbolo local: {}", name),
                span,
            );
        }
    }
}
