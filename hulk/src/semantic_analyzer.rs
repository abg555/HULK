use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::diagnostics::{Diagnostic, DiagnosticCollector};
use crate::symbol_table::{Symbol, SymbolKind, SymbolTable};
use crate::types::SemanticType;

#[derive(Clone)]
struct ParentLink {
    parent: String,
    span: Span,
}

#[derive(Debug)]
pub struct SemanticAnalysis {
    pub inferred_types: HashMap<NodeId, SemanticType>,
}

pub struct SemanticAnalyzer {
    symbols: SymbolTable,
    diagnostics: DiagnosticCollector,
    inferred_types: HashMap<NodeId, SemanticType>,
    current_return_type: Option<SemanticType>,
    type_parents: HashMap<String, ParentLink>,
    protocol_parents: HashMap<String, ParentLink>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        let mut analyzer = Self {
            symbols: SymbolTable::new(),
            diagnostics: DiagnosticCollector::new(),
            inferred_types: HashMap::new(),
            current_return_type: None,
            type_parents: HashMap::new(),
            protocol_parents: HashMap::new(),
        };
        analyzer.install_prelude();
        analyzer
    }

    pub fn analyze(mut self, program: &Program) -> Result<SemanticAnalysis, Vec<Diagnostic>> {
        self.collect_top_level(program);
        self.validate_hierarchies();

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
                        .map(|p| self.resolve_type_ref(p.types.as_ref(), func.body.span))
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

                    if let Some(TypeRef::Custom(parent_name)) = &typ.parent {
                        self.type_parents.insert(
                            typ.name.clone(),
                            ParentLink {
                                parent: parent_name.clone(),
                                span: self.type_decl_span(typ),
                            },
                        );
                    }
                }
                Item::Protocol(proto) => {
                    self.define_symbol(
                        &proto.name,
                        SymbolKind::Protocol,
                        SemanticType::Custom(proto.name.clone()),
                    );

                    if let Some(TypeRef::Custom(parent_name)) = proto.parent.as_deref() {
                        self.protocol_parents.insert(
                            proto.name.clone(),
                            ParentLink {
                                parent: parent_name.clone(),
                                span: self.protocol_decl_span(proto),
                            },
                        );
                    }
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
            let typ = self.resolve_type_ref(param.types.as_ref(), func.body.span);
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
            let parent_type = self.resolve_type_ref(Some(parent), self.type_decl_span(typ));
            if !matches!(parent_type, SemanticType::Custom(_)) {
                self.diagnostics.error(
                    format!("El padre de {} debe ser un tipo nombrado", typ.name),
                    self.type_decl_span(typ),
                );
            }
        }

        self.symbols.enter_scope();
        let mut field_names = HashSet::new();
        let mut method_names = HashSet::new();

        for field in &typ.fields {
            if !field_names.insert(field.name.clone()) {
                self.diagnostics.error(
                    format!("Campo duplicado en {}: {}", typ.name, field.name),
                    field.initializer.span,
                );
            }

            let init_ty = self.check_expr(&field.initializer);
            let declared = self.resolve_type_ref(field.type_annotation.as_ref(), field.initializer.span);
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
            if !method_names.insert(method.name.clone()) {
                self.diagnostics.error(
                    format!("Metodo duplicado en {}: {}", typ.name, method.name),
                    method.body.span,
                );
            }
            self.check_function_decl(method);
        }

        self.symbols.exit_scope();
    }

    fn check_protocol_decl(&mut self, proto: &ProtocolDecl) {
        if let Some(parent) = &proto.parent {
            let parent_type = self.resolve_type_ref(Some(parent.as_ref()), self.protocol_decl_span(proto));
            if !matches!(parent_type, SemanticType::Custom(_)) {
                self.diagnostics.error(
                    format!("El protocolo {} debe extender otro protocolo por nombre", proto.name),
                    self.protocol_decl_span(proto),
                );
            }
        }

        let mut signatures: HashMap<String, (usize, SemanticType)> = HashMap::new();
        for method in &proto.methods {
            let param_types = method
                .params
                .iter()
                .map(|p| self.resolve_type_ref(p.types.as_ref(), self.protocol_decl_span(proto)))
                .collect::<Vec<_>>();
            let ret = SemanticType::from_type_ref(&method.return_type);

            if let Some((expected_arity, expected_ret)) = signatures.get(&method.name) {
                if *expected_arity != param_types.len() || expected_ret != &ret {
                    self.diagnostics.error(
                        format!(
                            "Firma incompatible duplicada en protocolo {} para metodo {}",
                            proto.name, method.name
                        ),
                        self.protocol_decl_span(proto),
                    );
                }
            } else {
                signatures.insert(method.name.clone(), (param_types.len(), ret));
            }
        }
    }

    fn check_macro_decl(&mut self, macr: &MacroDecl) {
        self.symbols.enter_scope();
        for param in &macr.params {
            let typ = self.resolve_type_ref(param.type_info.as_ref(), macr.body.span);
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
                    let declared =
                        self.resolve_type_ref(binding.types.as_ref(), binding.initializer.span);
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
                    let param_ty = self.resolve_type_ref(param.types.as_ref(), expr.span);
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
                let _ = self.resolve_type_ref(Some(&is_expr.type_info), expr.span);
                SemanticType::Boolean
            }
            KindExpr::As(as_expr) => {
                self.check_expr(&as_expr.expression);
                self.resolve_type_ref(Some(&as_expr.type_info), expr.span)
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
                let typ = self.resolve_type_ref(type_restriction.as_ref(), Span { start: 0, end: 0 });
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

    fn resolve_type_ref(&mut self, type_ref: Option<&TypeRef>, span: Span) -> SemanticType {
        let Some(type_ref) = type_ref else {
            return SemanticType::Unknown;
        };

        let resolved = SemanticType::from_type_ref(type_ref);
        if let SemanticType::Custom(name) = &resolved
            && self.symbols.lookup(name).is_none()
        {
            self.diagnostics
                .error(format!("Tipo no definido: {}", name), span);
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

    fn install_prelude(&mut self) {
        // Builtins base para evitar falsos errores semanticos en programas validos.
        self.define_builtin_function("print", vec![SemanticType::Unknown], SemanticType::Unknown);
        self.define_builtin_function(
            "range",
            vec![SemanticType::Number, SemanticType::Number],
            SemanticType::Vector(Box::new(SemanticType::Number)),
        );
        self.define_builtin_function(
            "sqrt",
            vec![SemanticType::Number],
            SemanticType::Number,
        );
        self.define_builtin_function("sin", vec![SemanticType::Number], SemanticType::Number);
        self.define_builtin_function("cos", vec![SemanticType::Number], SemanticType::Number);
        self.define_builtin_function("exp", vec![SemanticType::Number], SemanticType::Number);
        self.define_builtin_function(
            "log",
            vec![SemanticType::Number, SemanticType::Number],
            SemanticType::Number,
        );
        self.define_builtin_function("rand", vec![], SemanticType::Number);
    }

    fn define_builtin_function(
        &mut self,
        name: &str,
        params: Vec<SemanticType>,
        ret: SemanticType,
    ) {
        let _ = self.symbols.define(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Function,
            typ: SemanticType::Function(params, Box::new(ret)),
        });
    }

    fn validate_hierarchies(&mut self) {
        for (child, link) in &self.type_parents {
            match self.symbols.lookup(&link.parent) {
                Some(symbol) if symbol.kind == SymbolKind::Type => {}
                Some(_) => {
                    self.diagnostics.error(
                        format!(
                            "{} hereda de {}, pero {} no es un tipo",
                            child, link.parent, link.parent
                        ),
                        link.span,
                    );
                }
                None => {
                    self.diagnostics.error(
                        format!("Tipo padre no definido: {} (usado por {})", link.parent, child),
                        link.span,
                    );
                }
            }
        }

        for (child, link) in &self.protocol_parents {
            match self.symbols.lookup(&link.parent) {
                Some(symbol) if symbol.kind == SymbolKind::Protocol => {}
                Some(_) => {
                    self.diagnostics.error(
                        format!(
                            "{} extiende {}, pero {} no es un protocolo",
                            child, link.parent, link.parent
                        ),
                        link.span,
                    );
                }
                None => {
                    self.diagnostics.error(
                        format!(
                            "Protocolo padre no definido: {} (usado por {})",
                            link.parent, child
                        ),
                        link.span,
                    );
                }
            }
        }

        let type_parents = self.type_parents.clone();
        let protocol_parents = self.protocol_parents.clone();
        self.detect_cycles(&type_parents, "herencia de tipos");
        self.detect_cycles(&protocol_parents, "extension de protocolos");
    }

    fn detect_cycles(&mut self, parents: &HashMap<String, ParentLink>, label: &str) {
        for start in parents.keys() {
            let mut visiting = HashSet::new();
            let mut current = start.as_str();

            while let Some(next) = parents.get(current) {
                if !visiting.insert(current.to_string()) {
                    self.diagnostics.error(
                        format!("Ciclo detectado en {} que involucra {}", label, current),
                        next.span,
                    );
                    break;
                }
                current = &next.parent;
            }
        }
    }

    fn type_decl_span(&self, typ: &TypeDecl) -> Span {
        if let Some(expr) = typ.parent_arg.first() {
            return expr.span;
        }
        if let Some(field) = typ.fields.first() {
            return field.initializer.span;
        }
        if let Some(method) = typ.methods.first() {
            return method.body.span;
        }
        Span { start: 0, end: 0 }
    }

    fn protocol_decl_span(&self, _proto: &ProtocolDecl) -> Span {
        Span { start: 0, end: 0 }
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
