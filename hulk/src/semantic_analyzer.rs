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

#[derive(Clone)]
struct TypeShape {
    ctor_params: Vec<SemanticType>,
    fields: HashMap<String, SemanticType>,
    methods: HashMap<String, SemanticType>,
    parent: Option<String>,
}

#[derive(Clone)]
struct ProtocolShape {
    methods: HashMap<String, SemanticType>,
    parent: Option<String>,
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
    type_shapes: HashMap<String, TypeShape>,
    protocol_shapes: HashMap<String, ProtocolShape>,
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
            type_shapes: HashMap::new(),
            protocol_shapes: HashMap::new(),
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
                        .map(|p| self.resolve_type_ref_silent(p.types.as_ref()))
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

                    self.type_shapes
                        .insert(typ.name.clone(), self.build_type_shape(typ));

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

                    self.protocol_shapes
                        .insert(proto.name.clone(), self.build_protocol_shape(proto));

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

    fn build_type_shape(&self, typ: &TypeDecl) -> TypeShape {
        let ctor_params = typ
            .param
            .iter()
            .map(|p| self.resolve_type_ref_silent(p.types.as_ref()))
            .collect::<Vec<_>>();

        let mut fields = HashMap::new();
        for field in &typ.fields {
            let field_ty = field
                .type_annotation
                .as_ref()
                .map(SemanticType::from_type_ref)
                .unwrap_or(SemanticType::Unknown);
            fields.insert(field.name.clone(), field_ty);
        }

        let mut methods = HashMap::new();
        for method in &typ.methods {
            let params = method
                .params
                .iter()
                .map(|p| self.resolve_type_ref_silent(p.types.as_ref()))
                .collect::<Vec<_>>();
            let ret = method
                .return_type
                .as_ref()
                .map(SemanticType::from_type_ref)
                .unwrap_or(SemanticType::Unknown);
            methods.insert(method.name.clone(), SemanticType::Function(params, Box::new(ret)));
        }

        let parent = match &typ.parent {
            Some(TypeRef::Custom(name)) => Some(name.clone()),
            _ => None,
        };

        TypeShape {
            ctor_params,
            fields,
            methods,
            parent,
        }
    }

    fn build_protocol_shape(&self, proto: &ProtocolDecl) -> ProtocolShape {
        let mut methods = HashMap::new();
        for method in &proto.methods {
            let params = method
                .params
                .iter()
                .map(|p| self.resolve_type_ref_silent(p.types.as_ref()))
                .collect::<Vec<_>>();
            let ret = SemanticType::from_type_ref(&method.return_type);
            methods.insert(method.name.clone(), SemanticType::Function(params, Box::new(ret)));
        }

        let parent = match proto.parent.as_deref() {
            Some(TypeRef::Custom(name)) => Some(name.clone()),
            _ => None,
        };

        ProtocolShape { methods, parent }
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
            && !self.is_compatible_type(expected, &body_ty)
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
            let parent_ok = matches!(parent_type, SemanticType::Custom(name)
                if self.symbols.lookup(&name).map(|symbol| symbol.kind) == Some(SymbolKind::Type));
            if !parent_ok {
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

            if let Some(parent_member) = self.lookup_member_type_in_parent_chain(&typ.name, &field.name) {
                self.diagnostics.error(
                    format!(
                        "El campo {} en {} colisiona con miembro heredado de tipo {}",
                        field.name, typ.name, parent_member
                    ),
                    field.initializer.span,
                );
            }

            let init_ty = self.check_expr(&field.initializer);
            let declared = self.resolve_type_ref(field.type_annotation.as_ref(), field.initializer.span);
            if !self.is_compatible_type(&declared, &init_ty) {
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

            self.validate_method_override(typ, method);
            self.check_function_decl(method);
        }

        self.symbols.exit_scope();
    }

    fn check_protocol_decl(&mut self, proto: &ProtocolDecl) {
        if let Some(parent) = &proto.parent {
            let parent_type = self.resolve_type_ref(Some(parent.as_ref()), self.protocol_decl_span(proto));
            let parent_ok = matches!(parent_type, SemanticType::Custom(name)
                if self.symbols.lookup(&name).map(|symbol| symbol.kind) == Some(SymbolKind::Protocol));
            if !parent_ok {
                self.diagnostics.error(
                    format!("El protocolo {} debe extender otro protocolo por nombre", proto.name),
                    self.protocol_decl_span(proto),
                );
            }
        }

        if let Some(TypeRef::Custom(parent_name)) = proto.parent.as_deref() {
            if !self.protocol_conforms_to_protocol(&proto.name, parent_name) {
                self.diagnostics.error(
                    format!("El protocolo {} no es compatible con su padre {}", proto.name, parent_name),
                    self.protocol_decl_span(proto),
                );
            }
        }

        if let Some(TypeRef::Custom(parent_name)) = proto.parent.as_deref() {
            if !self.protocol_conforms_to_protocol(&proto.name, parent_name) {
                self.diagnostics.error(
                    format!("El protocolo {} no es compatible con su padre {}", proto.name, parent_name),
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
                        if !self.is_compatible_type(expected, actual) {
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
                    if !self.is_compatible_type(&declared, &init_ty) {
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

                let then_ty = if let Some((name, narrowed_type)) =
                    self.extract_is_narrowing(&if_expr.condition, expr.span)
                {
                    self.symbols.enter_scope();
                    self.define_local(&name, SymbolKind::Variable, narrowed_type, expr.span);
                    let ty = self.check_expr(&if_expr.then_branch);
                    self.symbols.exit_scope();
                    ty
                } else {
                    self.check_expr(&if_expr.then_branch)
                };
                for (elif_cond, elif_body) in &if_expr.elif_branches {
                    let elif_cond_ty = self.check_expr(elif_cond);
                    self.expect_type(expr.span, &elif_cond_ty, &SemanticType::Boolean, "condicion de elif");
                    let _ = self.check_expr(elif_body);
                }

                let else_ty = self.check_expr(&if_expr.else_branch);
                if self.is_compatible_type(&then_ty, &else_ty) {
                    then_ty
                } else if self.is_compatible_type(&else_ty, &then_ty) {
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
                if !Self::is_assignable_target(&assign.target.kind) {
                    self.diagnostics.error(
                        "El lado izquierdo de ':=' debe ser variable, miembro o indexacion",
                        assign.target.span,
                    );
                }

                let target_ty = self.check_expr(&assign.target);
                let value_ty = self.check_expr(&assign.value);
                if !self.is_compatible_type(&target_ty, &value_ty) {
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
                let obj_ty = self.check_expr(&member.object);
                self.resolve_member_type(&obj_ty, &member.field, expr.span)
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
                        if !self.is_compatible_type(&element_ty, &current)
                            && !self.is_compatible_type(&current, &element_ty)
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

                let arg_types = new_expr
                    .arguments
                    .iter()
                    .map(|arg| self.check_expr(arg))
                    .collect::<Vec<_>>();

                if let Some(shape) = self.type_shapes.get(&new_expr.type_name) {
                    if shape.ctor_params.len() != arg_types.len() {
                        self.diagnostics.error(
                            format!(
                                "Constructor de {} espera {} argumentos y recibio {}",
                                new_expr.type_name,
                                shape.ctor_params.len(),
                                arg_types.len()
                            ),
                            expr.span,
                        );
                    }

                    for (idx, (expected, actual)) in
                        shape.ctor_params.iter().zip(arg_types.iter()).enumerate()
                    {
                        if !self.is_compatible_type(expected, actual) {
                            self.diagnostics.error(
                                format!(
                                    "Argumento {} incompatible en constructor de {}: esperado {}, recibido {}",
                                    idx + 1,
                                    new_expr.type_name,
                                    expected,
                                    actual
                                ),
                                expr.span,
                            );
                        }
                    }
                }
                SemanticType::Custom(new_expr.type_name.clone())
            }
            KindExpr::Is(is_expr) => {
                let expression_ty = self.check_expr(&is_expr.expression);
                let target_ty = self.resolve_type_ref(Some(&is_expr.type_info), expr.span);
                if !self.is_compatible_type(&target_ty, &expression_ty)
                    && !self.is_compatible_type(&expression_ty, &target_ty)
                {
                    self.diagnostics.error(
                        format!(
                            "Chequeo 'is' incompatible: {} no puede contrastarse con {}",
                            expression_ty, target_ty
                        ),
                        expr.span,
                    );
                }
                SemanticType::Boolean
            }
            KindExpr::As(as_expr) => {
                let expression_ty = self.check_expr(&as_expr.expression);
                let target_ty = self.resolve_type_ref(Some(&as_expr.type_info), expr.span);
                if !self.is_compatible_type(&target_ty, &expression_ty)
                    && !self.is_compatible_type(&expression_ty, &target_ty)
                {
                    self.diagnostics.error(
                        format!(
                            "Cast 'as' incompatible: no se puede convertir {} a {}",
                            expression_ty, target_ty
                        ),
                        expr.span,
                    );
                }
                target_ty
            }
            KindExpr::Match(match_expr) => {
                let scrutinee_ty = self.check_expr(&match_expr.expression);
                let scrutinee_name = match &match_expr.expression.kind {
                    KindExpr::Variable(var) => Some(var.name.as_str()),
                    _ => None,
                };
                let mut merged = SemanticType::Unknown;
                let mut saw_true_case = false;
                let mut saw_false_case = false;
                let mut saw_default_case = false;

                for case in &match_expr.cases {
                    if saw_default_case {
                        self.diagnostics.error(
                            "Caso inalcanzable: hay un default previo en match",
                            expr.span,
                        );
                    }

                    self.symbols.enter_scope();
                    self.check_pattern(&case.pattern, &scrutinee_ty, scrutinee_name, expr.span);
                    let case_ty = self.check_expr(&case.body);
                    self.symbols.exit_scope();

                    match &case.pattern {
                        Pattern::Literal(LiteralValue::Bool(true)) => {
                            if saw_true_case {
                                self.diagnostics.error(
                                    "Patron duplicado: case true repetido",
                                    expr.span,
                                );
                            }
                            saw_true_case = true;
                        }
                        Pattern::Literal(LiteralValue::Bool(false)) => {
                            if saw_false_case {
                                self.diagnostics.error(
                                    "Patron duplicado: case false repetido",
                                    expr.span,
                                );
                            }
                            saw_false_case = true;
                        }
                        Pattern::Default => {
                            if saw_default_case {
                                self.diagnostics.error(
                                    "Patron duplicado: multiple default en match",
                                    expr.span,
                                );
                            }
                            saw_default_case = true;
                        }
                        _ => {}
                    }

                    if matches!(merged, SemanticType::Unknown) {
                        merged = case_ty;
                    } else if !self.is_compatible_type(&merged, &case_ty)
                        && !self.is_compatible_type(&case_ty, &merged)
                    {
                        merged = SemanticType::Unknown;
                    }
                }

                if matches!(scrutinee_ty, SemanticType::Boolean)
                    && !saw_default_case
                    && !(saw_true_case && saw_false_case)
                {
                    self.diagnostics.error(
                        "Match sobre Boolean no exhaustivo: faltan casos true/false o default",
                        expr.span,
                    );
                }

                merged
            }
        };

        self.inferred_types.insert(expr.id, inferred.clone());
        inferred
    }

    fn check_pattern(
        &mut self,
        pattern: &Pattern,
        scrutinee_ty: &SemanticType,
        scrutinee_name: Option<&str>,
        span: Span,
    ) {
        match pattern {
            Pattern::Identifier {
                name,
                type_restriction,
            } => {
                let restricted = self.resolve_type_ref(type_restriction.as_ref(), span);
                let bound_type = if type_restriction.is_some() {
                    if !self.is_compatible_type(&restricted, scrutinee_ty)
                        && !self.is_compatible_type(scrutinee_ty, &restricted)
                    {
                        self.diagnostics.error(
                            format!(
                                "Pattern incompatible: se esperaba {}, se obtuvo {}",
                                scrutinee_ty, restricted
                            ),
                            span,
                        );
                    }
                    restricted
                } else {
                    scrutinee_ty.clone()
                };

                self.define_local(name, SymbolKind::Variable, bound_type.clone(), span);

                if let Some(scrutinee_name) = scrutinee_name
                    && scrutinee_name != name
                {
                    self.define_local(
                        scrutinee_name,
                        SymbolKind::Variable,
                        bound_type,
                        span,
                    );
                }
            }
            Pattern::Binary { left, right, .. } => {
                self.check_pattern(left, scrutinee_ty, scrutinee_name, span);
                self.check_pattern(right, scrutinee_ty, scrutinee_name, span);
            }
            Pattern::Unary { operand, .. } => {
                self.check_pattern(operand, scrutinee_ty, scrutinee_name, span)
            }
            Pattern::Literal(lit) => {
                let lit_ty = self.literal_type(lit);
                if !self.is_compatible_type(scrutinee_ty, &lit_ty)
                    && !self.is_compatible_type(&lit_ty, scrutinee_ty)
                {
                    self.diagnostics.error(
                        format!(
                            "Literal de patron incompatible: {} no coincide con {}",
                            lit_ty, scrutinee_ty
                        ),
                        span,
                    );
                }
            }
            Pattern::Default => {}
        }
    }

    fn extract_is_narrowing(&mut self, condition: &Expr, span: Span) -> Option<(String, SemanticType)> {
        let KindExpr::Is(is_expr) = &condition.kind else {
            return None;
        };

        let KindExpr::Variable(var) = &is_expr.expression.kind else {
            return None;
        };

        let current_ty = self.check_expr(&is_expr.expression);
        let narrowed_ty = self.resolve_type_ref(Some(&is_expr.type_info), span);
        if self.is_compatible_type(&narrowed_ty, &current_ty)
            || self.is_compatible_type(&current_ty, &narrowed_ty)
        {
            Some((var.name.clone(), narrowed_ty))
        } else {
            None
        }
    }

    fn literal_type(&self, literal: &LiteralValue) -> SemanticType {
        match literal {
            LiteralValue::Number(_) => SemanticType::Number,
            LiteralValue::String(_) => SemanticType::String,
            LiteralValue::Bool(_) => SemanticType::Boolean,
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
                if !self.is_compatible_type(&left, &right) && !self.is_compatible_type(&right, &left) {
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

    fn resolve_type_ref_silent(&self, type_ref: Option<&TypeRef>) -> SemanticType {
        type_ref
            .map(SemanticType::from_type_ref)
            .unwrap_or(SemanticType::Unknown)
    }

    fn is_compatible_type(&self, expected: &SemanticType, actual: &SemanticType) -> bool {
        if matches!(expected, SemanticType::Unknown) || matches!(actual, SemanticType::Unknown) {
            return true;
        }

        if expected == actual {
            return true;
        }

        match (expected, actual) {
            (SemanticType::Vector(expected_inner), SemanticType::Vector(actual_inner)) => {
                self.is_compatible_type(expected_inner, actual_inner)
            }
            (
                SemanticType::Function(expected_params, expected_ret),
                SemanticType::Function(actual_params, actual_ret),
            ) => {
                expected_params.len() == actual_params.len()
                    && expected_params
                        .iter()
                        .zip(actual_params.iter())
                        .all(|(expected_param, actual_param)| {
                            self.is_compatible_type(actual_param, expected_param)
                        })
                    && self.is_compatible_type(expected_ret, actual_ret)
            }
            (SemanticType::Custom(expected_name), SemanticType::Custom(actual_name)) => {
                self.custom_type_compatible(expected_name, actual_name)
            }
            _ => false,
        }
    }

    fn custom_type_compatible(&self, expected_name: &str, actual_name: &str) -> bool {
        if expected_name == actual_name {
            return true;
        }

        let Some(expected_symbol) = self.symbols.lookup(expected_name) else {
            return false;
        };
        let Some(actual_symbol) = self.symbols.lookup(actual_name) else {
            return false;
        };

        match (expected_symbol.kind, actual_symbol.kind) {
            (SymbolKind::Type, SymbolKind::Type) => self.type_is_subtype_of(actual_name, expected_name),
            (SymbolKind::Protocol, SymbolKind::Type) => {
                self.type_conforms_to_protocol(actual_name, expected_name)
            }
            (SymbolKind::Protocol, SymbolKind::Protocol) => {
                self.protocol_conforms_to_protocol(actual_name, expected_name)
            }
            _ => false,
        }
    }

    fn type_is_subtype_of(&self, actual_name: &str, expected_name: &str) -> bool {
        if actual_name == expected_name {
            return true;
        }

        let mut current = Some(actual_name.to_string());
        let mut visited = HashSet::new();

        while let Some(name) = current {
            if !visited.insert(name.clone()) {
                break;
            }

            let Some(shape) = self.type_shapes.get(&name) else {
                break;
            };

            match &shape.parent {
                Some(parent) if parent == expected_name => return true,
                Some(parent) => current = Some(parent.clone()),
                None => break,
            }
        }

        false
    }

    fn type_conforms_to_protocol(&self, type_name: &str, protocol_name: &str) -> bool {
        let Some(required_methods) = self.collect_protocol_methods(protocol_name) else {
            return false;
        };

        for (method_name, expected_signature) in required_methods {
            let Some(actual_signature) = self.lookup_member_type(type_name, &method_name) else {
                return false;
            };

            let (
                SemanticType::Function(expected_params, expected_ret),
                SemanticType::Function(actual_params, actual_ret),
            ) = (&expected_signature, &actual_signature)
            else {
                return false;
            };

            if expected_params.len() != actual_params.len() {
                return false;
            }

            if !expected_params
                .iter()
                .zip(actual_params.iter())
                .all(|(expected_param, actual_param)| {
                    self.is_compatible_type(actual_param, expected_param)
                })
            {
                return false;
            }

            if !self.is_compatible_type(expected_ret, actual_ret) {
                return false;
            }
        }

        true
    }

    fn protocol_conforms_to_protocol(&self, actual_name: &str, expected_name: &str) -> bool {
        if actual_name == expected_name {
            return true;
        }

        if self.protocol_extends(actual_name, expected_name) {
            return true;
        }

        let Some(expected_methods) = self.collect_protocol_methods(expected_name) else {
            return false;
        };
        let Some(actual_methods) = self.collect_protocol_methods(actual_name) else {
            return false;
        };

        for (method_name, expected_signature) in expected_methods {
            let Some(actual_signature) = actual_methods.get(&method_name) else {
                return false;
            };

            let (
                SemanticType::Function(expected_params, expected_ret),
                SemanticType::Function(actual_params, actual_ret),
            ) = (&expected_signature, actual_signature)
            else {
                return false;
            };

            if expected_params.len() != actual_params.len() {
                return false;
            }

            if !expected_params
                .iter()
                .zip(actual_params.iter())
                .all(|(expected_param, actual_param)| {
                    self.is_compatible_type(actual_param, expected_param)
                })
            {
                return false;
            }

            if !self.is_compatible_type(expected_ret, actual_ret) {
                return false;
            }
        }

        true
    }

    fn protocol_extends(&self, actual_name: &str, expected_name: &str) -> bool {
        let mut current = Some(actual_name.to_string());
        let mut visited = HashSet::new();

        while let Some(name) = current {
            if !visited.insert(name.clone()) {
                break;
            }

            let Some(shape) = self.protocol_shapes.get(&name) else {
                break;
            };

            match &shape.parent {
                Some(parent) if parent == expected_name => return true,
                Some(parent) => current = Some(parent.clone()),
                None => break,
            }
        }

        false
    }

    fn collect_protocol_methods(&self, protocol_name: &str) -> Option<HashMap<String, SemanticType>> {
        let mut collected = HashMap::new();
        self.collect_protocol_methods_recursive(protocol_name, &mut collected)?;
        Some(collected)
    }

    fn collect_protocol_methods_recursive(
        &self,
        protocol_name: &str,
        collected: &mut HashMap<String, SemanticType>,
    ) -> Option<()> {
        let shape = self.protocol_shapes.get(protocol_name)?;

        if let Some(parent) = &shape.parent {
            self.collect_protocol_methods_recursive(parent, collected)?;
        }

        for (name, signature) in &shape.methods {
            collected.insert(name.clone(), signature.clone());
        }

        Some(())
    }

    fn resolve_member_type(&mut self, object_type: &SemanticType, member: &str, span: Span) -> SemanticType {
        let SemanticType::Custom(type_name) = object_type else {
            self.diagnostics.error(
                format!("Acceso a miembro {} sobre valor no estructurado ({})", member, object_type),
                span,
            );
            return SemanticType::Unknown;
        };

        if let Some(member_type) = self.lookup_member_type(type_name, member) {
            return member_type;
        }

        self.diagnostics.error(
            format!("El tipo {} no define el miembro {}", type_name, member),
            span,
        );
        SemanticType::Unknown
    }

    fn lookup_member_type(&self, type_name: &str, member: &str) -> Option<SemanticType> {
        if let Some(signature) = self.lookup_protocol_member_type(type_name, member) {
            return Some(signature);
        }

        let mut current = Some(type_name.to_string());
        let mut visited = HashSet::new();

        while let Some(name) = current {
            if !visited.insert(name.clone()) {
                break;
            }

            let shape = self.type_shapes.get(&name)?;
            if let Some(field_ty) = shape.fields.get(member) {
                return Some(field_ty.clone());
            }
            if let Some(method_ty) = shape.methods.get(member) {
                return Some(method_ty.clone());
            }

            current = shape.parent.clone();
        }

        None
    }

    fn lookup_member_type_in_parent_chain(&self, type_name: &str, member: &str) -> Option<SemanticType> {
        let mut current = self
            .type_shapes
            .get(type_name)
            .and_then(|shape| shape.parent.clone());
        let mut visited = HashSet::new();

        while let Some(name) = current {
            if !visited.insert(name.clone()) {
                break;
            }

            let shape = self.type_shapes.get(&name)?;
            if let Some(field_ty) = shape.fields.get(member) {
                return Some(field_ty.clone());
            }
            if let Some(method_ty) = shape.methods.get(member) {
                return Some(method_ty.clone());
            }

            current = shape.parent.clone();
        }

        None
    }

    fn validate_method_override(&mut self, typ: &TypeDecl, method: &FunctionDecl) {
        let child_signature = SemanticType::Function(
            method
                .params
                .iter()
                .map(|p| self.resolve_type_ref_silent(p.types.as_ref()))
                .collect::<Vec<_>>(),
            Box::new(
                method
                    .return_type
                    .as_ref()
                    .map(SemanticType::from_type_ref)
                    .unwrap_or(SemanticType::Unknown),
            ),
        );

        if let Some(parent_signature) = self.lookup_member_type_in_parent_chain(&typ.name, &method.name) {
            let parent_is_method = matches!(parent_signature, SemanticType::Function(_, _));
            if !parent_is_method {
                self.diagnostics.error(
                    format!(
                        "El metodo {} en {} colisiona con un campo heredado",
                        method.name, typ.name
                    ),
                    method.body.span,
                );
                return;
            }

            if !self.is_compatible_type(&parent_signature, &child_signature) {
                self.diagnostics.error(
                    format!(
                        "Override incompatible en {}.{}: firma hija {} no es compatible con firma padre {}",
                        typ.name, method.name, child_signature, parent_signature
                    ),
                    method.body.span,
                );
            }
        }
    }

    fn lookup_protocol_member_type(&self, protocol_name: &str, member: &str) -> Option<SemanticType> {
        let mut current = Some(protocol_name.to_string());
        let mut visited = HashSet::new();

        while let Some(name) = current {
            if !visited.insert(name.clone()) {
                break;
            }

            let shape = self.protocol_shapes.get(&name)?;
            if let Some(signature) = shape.methods.get(member) {
                return Some(signature.clone());
            }

            current = shape.parent.clone();
        }

        None
    }

    fn is_assignable_target(kind: &KindExpr) -> bool {
        matches!(
            kind,
            KindExpr::Variable(_) | KindExpr::MemberAccess(_) | KindExpr::Index(_)
        )
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
