use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::semantic::symbol_table::SymbolKind;
use crate::semantic::types::SemanticType;

use super::SemanticAnalyzer;

impl SemanticAnalyzer {
    /// Valida una funcion o metodo dentro de su ambito local.
    pub(super) fn check_function_decl(&mut self, func: &FunctionDecl) {
        self.enter_scope();

        if let Some(type_name) = &self.current_type_context {
            self.define_local(
                "self",
                SymbolKind::Variable,
                SemanticType::Custom(type_name.clone()),
                func.body.span,
            );
            self.mark_local_readonly("self", "self es de solo lectura");
        }

        let inferred_param_types = if let Some(type_name) = &self.current_type_context {
            self.inferred_method_params
                .get(type_name)
                .and_then(|methods| methods.get(&func.name))
                .cloned()
                .unwrap_or_else(|| self.infer_param_types(func))
        } else {
            self.inferred_function_params
                .get(&func.name)
                .cloned()
                .unwrap_or_else(|| self.infer_param_types(func))
        };
        let resolved_param_types = func
            .params
            .iter()
            .map(|param| {
                let base_type = param
                    .types
                    .as_ref()
                    .map(|type_ref| SemanticType::from_type_ref(type_ref))
                    .unwrap_or_else(|| {
                        inferred_param_types
                            .get(&param.name)
                            .cloned()
                            .unwrap_or(SemanticType::Unknown)
                    });
                if param.is_variadic {
                    SemanticType::Vector(Box::new(base_type))
                } else {
                    base_type
                }
            })
            .collect::<Vec<_>>();

        let resolved_return_type = func
            .return_type
            .as_ref()
            .map(SemanticType::from_type_ref)
            .unwrap_or_else(|| {
                if let Some(type_name) = &self.current_type_context {
                    self.inferred_method_returns
                        .get(type_name)
                        .and_then(|methods| methods.get(&func.name))
                        .cloned()
                        .unwrap_or(SemanticType::Unknown)
                } else {
                    self.inferred_function_returns
                        .get(&func.name)
                        .cloned()
                        .unwrap_or(SemanticType::Unknown)
                }
            });

        let _ = self.symbols.update_type(
            &func.name,
            SemanticType::Function(resolved_param_types, Box::new(resolved_return_type)),
        );

        for param in &func.params {
            let base_type = param
                .types
                .as_ref()
                .map(|type_ref| self.resolve_type_ref(Some(type_ref), func.body.span))
                .unwrap_or_else(|| {
                    inferred_param_types
                        .get(&param.name)
                        .cloned()
                        .unwrap_or(SemanticType::Unknown)
                });
            let typ = if param.is_variadic {
                SemanticType::Vector(Box::new(base_type))
            } else {
                base_type
            };
            self.define_local(&param.name, SymbolKind::Variable, typ, func.body.span);
        }

        let prev_return = self.current_return_type.clone();
        self.current_return_type = func.return_type.as_ref().map(SemanticType::from_type_ref);

        let body_ty = self.check_expr(&func.body);
        if let Some(expected) = &self.current_return_type {
            if !self.guarantees_value(&func.body) {
                self.diagnostics.error(
                    format!(
                        "La funcion {} con retorno {} no garantiza valor en todos los caminos",
                        func.name, expected
                    ),
                    func.body.span,
                );
            } else if !self.is_compatible_type(expected, &body_ty) {
                self.diagnostics.error(
                    format!(
                        "La funcion {} retorna {}, se esperaba {}",
                        func.name, body_ty, expected
                    ),
                    func.body.span,
                );
            }
        }

        self.current_return_type = prev_return;
        self.exit_scope();
    }

    /// Infiere y valida el tipo de una expresion completa.
    pub(super) fn check_expr(&mut self, expr: &Expr) -> SemanticType {
        let inferred = match &expr.kind {
            KindExpr::Literal(lit) => match lit.value {
                LiteralValue::Number(_) => SemanticType::Number,
                LiteralValue::String(_) => SemanticType::String,
                LiteralValue::Bool(_) => SemanticType::Boolean,
            },
            KindExpr::Variable(var) => {
                if var.name == "self" && self.current_type_context.is_none() {
                    self.diagnostics
                        .error("'self' solo es valido dentro de metodos de tipo", expr.span);
                    SemanticType::Unknown
                } else if let Some(symbol) = self.symbols.lookup(&var.name) {
                    let symbol_kind = symbol.kind;
                    let symbol_type = symbol.typ.clone();
                    if symbol_kind == SymbolKind::Variable
                        && !self.is_definitely_assigned(&var.name)
                    {
                        self.diagnostics.error(
                            format!("La variable {} puede no estar inicializada", var.name),
                            expr.span,
                        );
                    }
                    symbol_type
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
                        self.expect_type(
                            expr.span,
                            &right,
                            &SemanticType::Boolean,
                            "operador '!' ",
                        );
                        SemanticType::Boolean
                    }
                }
            }
            KindExpr::Call(call) => {
                if let KindExpr::Variable(var) = &call.callee.kind
                    && self
                        .symbols
                        .lookup(&var.name)
                        .is_some_and(|symbol| symbol.kind == SymbolKind::Type)
                {
                    let arg_types = call
                        .arguments
                        .iter()
                        .map(|arg| self.check_expr(arg))
                        .collect::<Vec<_>>();
                    self.check_constructor_call(&var.name, &arg_types, expr.span)
                } else {
                    let callee = self.check_expr(&call.callee);
                    let arg_types = call
                        .arguments
                        .iter()
                        .map(|arg| self.check_expr(arg))
                        .collect::<Vec<_>>();

                    match callee {
                        SemanticType::Function(params, ret) => {
                            let mut resolved_ret =
                                self.check_callable_signature(&params, *ret, &arg_types, expr.span);
                            if let KindExpr::Variable(var) = &call.callee.kind {
                                if var.name == "print" && !arg_types.is_empty() {
                                    resolved_ret = arg_types[0].clone();
                                }
                            }
                            resolved_ret
                        }
                        SemanticType::Custom(type_name) => {
                            if let Some(invoke_signature) =
                                self.lookup_functor_invoke_type(&type_name)
                                && let SemanticType::Function(params, ret) = invoke_signature
                            {
                                self.check_callable_signature(&params, *ret, &arg_types, expr.span)
                            } else {
                                self.diagnostics.error(
                                    "Intento de llamada sobre un valor no invocable",
                                    expr.span,
                                );
                                SemanticType::Unknown
                            }
                        }
                        _ => {
                            self.diagnostics
                                .error("Intento de llamada sobre un valor no invocable", expr.span);
                            SemanticType::Unknown
                        }
                    }
                }
            }
            KindExpr::BaseCall(call) => self.check_base_call(call, expr.span),
            KindExpr::MacroCall(call) => {
                if self.symbols.lookup(&call.name).is_none() {
                    self.diagnostics
                        .error(format!("Macro no definida: {}", call.name), expr.span);
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
                // Implement `let a = x, b = y in ...` as nested lets evaluated left-to-right.
                // Create an initial let scope that will be the outermost of the nested chain.
                self.enter_scope();
                let body_inferable = let_expr
                    .bindings
                    .iter()
                    .filter(|binding| binding.types.is_none())
                    .map(|binding| binding.name.clone())
                    .collect::<HashSet<_>>();

                let mut entered_scopes = 1usize; // we entered one scope above

                for (idx, binding) in let_expr.bindings.iter().enumerate() {
                    let initializer_guaranteed = self.guarantees_value(&binding.initializer);
                    if !initializer_guaranteed {
                        self.diagnostics.error(
                            format!(
                                "El inicializador de {} no garantiza valor en todos los caminos",
                                binding.name
                            ),
                            binding.initializer.span,
                        );
                    }

                    // Evaluate initializer in the current (innermost) scope so it can see
                    // previously-defined bindings (they live in outer scopes of this chain).
                    let init_ty = self.check_expr(&binding.initializer);
                    let declared =
                        self.resolve_type_ref(binding.types.as_ref(), binding.initializer.span);
                    let final_ty =
                        if binding.types.is_none() && matches!(init_ty, SemanticType::Unknown) {
                            let mut requirements: HashMap<String, super::SymbolRequirements> =
                                HashMap::new();
                            self.collect_inference_requirements(
                                &let_expr.body,
                                &body_inferable,
                                &mut Vec::new(),
                                &mut requirements,
                            );
                            self.synthesize_inferred_type(
                                &binding.name,
                                requirements.remove(&binding.name).unwrap_or_default(),
                                binding.initializer.span,
                            )
                        } else if matches!(declared, SemanticType::Unknown) {
                            init_ty.clone()
                        } else {
                            declared.clone()
                        };

                    if binding.types.is_some() && !self.is_compatible_type(&declared, &init_ty) {
                        self.diagnostics.error(
                            format!(
                                "Binding {} incompatible: se esperaba {}, se obtuvo {}",
                                binding.name, declared, init_ty
                            ),
                            binding.initializer.span,
                        );
                    }

                    // Define the binding in the current scope (so it's visible to inner scopes).
                    self.define_local_with_state(
                        &binding.name,
                        SymbolKind::Variable,
                        final_ty,
                        binding.initializer.span,
                        initializer_guaranteed,
                    );

                    // If there are more bindings remaining, create a new inner scope so the
                    // next binding will live in an inner scope that shadows the current one.
                    if idx + 1 < let_expr.bindings.len() {
                        self.enter_scope();
                        entered_scopes += 1;
                    }
                }

                // Body is evaluated in the innermost scope created above.
                let body_ty = self.check_expr(&let_expr.body);

                // Exit all scopes we entered for this let expression.
                for _ in 0..entered_scopes {
                    self.exit_scope();
                }

                body_ty
            }
            KindExpr::Block(block) => {
                self.enter_scope();
                let mut last = SemanticType::Unknown;
                let mut found_non_terminating = false;
                for sub in &block.expressions {
                    if found_non_terminating {
                        self.diagnostics.error(
                            "Expresion inalcanzable: hay una expresion previa que no termina",
                            sub.span,
                        );
                    }
                    last = self.check_expr(sub);
                    if self.definitely_non_terminating(sub) {
                        found_non_terminating = true;
                    }
                }
                self.exit_scope();
                last
            }
            KindExpr::If(if_expr) => self.check_if_expr(if_expr, expr.span),
            KindExpr::While(while_expr) => {
                let min_iterations = self.while_min_iterations(&while_expr.condition);
                if matches!(min_iterations, Some(0)) {
                    self.diagnostics.error(
                        "Cuerpo de while inalcanzable: la condicion es siempre false",
                        while_expr.body.span,
                    );
                }
                let cond_ty = self.check_expr(&while_expr.condition);
                self.expect_type(
                    expr.span,
                    &cond_ty,
                    &SemanticType::Boolean,
                    "condicion de while",
                );
                let state_after_condition_eval = self.assigned_scopes.clone();

                let mut loop_state = state_after_condition_eval.clone();
                let mut body_ty = SemanticType::Unknown;
                for _ in 0..Self::LOOP_FIXPOINT_MAX_ITERS {
                    self.assigned_scopes = loop_state.clone();
                    body_ty = self.check_expr(&while_expr.body);
                    let next_state = self.assigned_scopes.clone();
                    if next_state == loop_state {
                        break;
                    }
                    loop_state = next_state;
                }

                self.assigned_scopes = match min_iterations {
                    Some(0) => state_after_condition_eval,
                    Some(_) => loop_state,
                    None => self.intersect_definite_assignment_states(
                        &state_after_condition_eval,
                        &loop_state,
                    ),
                };
                body_ty
            }
            KindExpr::For(for_expr) => {
                let min_iterations = self.iterable_min_iterations(&for_expr.iterable);
                let iterable_ty = self.check_expr(&for_expr.iterable);
                let state_after_iterable_eval = self.assigned_scopes.clone();

                let element_ty = match iterable_ty {
                    SemanticType::Vector(inner) => *inner,
                    SemanticType::Custom(ref type_name) => {
                        if self.type_conforms_to_protocol(type_name, "Iterable") {
                            self.iterable_element_type(type_name)
                                .unwrap_or(SemanticType::Unknown)
                        } else {
                            self.diagnostics.error(
                                format!(
                                    "El for requiere un vector o un tipo que implemente el protocolo Iterable, se obtuvo {}",
                                    iterable_ty
                                ),
                                expr.span,
                            );
                            SemanticType::Unknown
                        }
                    }
                    other => {
                        self.diagnostics.error(
                            format!(
                                "El for requiere un vector o un tipo que implemente el protocolo Iterable, se obtuvo {}",
                                other
                            ),
                            expr.span,
                        );
                        SemanticType::Unknown
                    }
                };

                let mut loop_state = state_after_iterable_eval.clone();
                let mut body_ty = SemanticType::Unknown;
                for _ in 0..Self::LOOP_FIXPOINT_MAX_ITERS {
                    self.assigned_scopes = loop_state.clone();
                    self.enter_scope();
                    self.define_local(
                        &for_expr.variable,
                        SymbolKind::Variable,
                        element_ty.clone(),
                        expr.span,
                    );
                    self.mark_local_readonly(&for_expr.variable, "iterador de for");
                    body_ty = self.check_expr(&for_expr.body);
                    self.exit_scope();
                    let next_state = self.assigned_scopes.clone();
                    if next_state == loop_state {
                        break;
                    }
                    loop_state = next_state;
                }

                self.assigned_scopes = match min_iterations {
                    Some(0) => state_after_iterable_eval,
                    Some(_) => loop_state,
                    None => self.intersect_definite_assignment_states(
                        &state_after_iterable_eval,
                        &loop_state,
                    ),
                };
                body_ty
            }
            KindExpr::Assign(assign) => {
                if !Self::is_assignable_target(&assign.target.kind) {
                    self.diagnostics.error(
                        "El lado izquierdo de ':=' debe ser variable, miembro o indexacion",
                        assign.target.span,
                    );
                }

                let target_ty = self.check_assignment_target(&assign.target);
                let value_ty = self.check_expr(&assign.value);
                if !self.guarantees_value(&assign.value) {
                    self.diagnostics.error(
                        "La expresion asignada no garantiza valor",
                        assign.value.span,
                    );
                }
                if !self.is_compatible_type(&target_ty, &value_ty) {
                    self.diagnostics.error(
                        format!(
                            "Asignacion incompatible: se esperaba {}, se obtuvo {}",
                            target_ty, value_ty
                        ),
                        expr.span,
                    );
                }

                if let KindExpr::Variable(var) = &assign.target.kind {
                    if let Some(reason) = self.readonly_reason(&var.name) {
                        self.diagnostics.error(
                            format!("No se puede asignar a {}: {}", var.name, reason),
                            assign.target.span,
                        );
                    } else {
                        self.mark_assigned(&var.name);
                    }
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
                self.expect_type(
                    expr.span,
                    &idx_ty,
                    &SemanticType::Number,
                    "indice de arreglo",
                );
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
                        element_ty = self.common_supertype(&element_ty, &current);
                    }
                    SemanticType::Vector(Box::new(element_ty))
                }
            }
            KindExpr::ArrayComprehension(comp) => {
                let iter_ty = self.check_expr(&comp.iterable);
                self.enter_scope();
                let loop_ty = match iter_ty {
                    SemanticType::Vector(inner) => *inner,
                    _ => SemanticType::Unknown,
                };
                self.define_local(&comp.variable, SymbolKind::Variable, loop_ty, expr.span);
                let elem_ty = self.check_expr(&comp.element);
                self.exit_scope();
                SemanticType::Vector(Box::new(elem_ty))
            }
            KindExpr::Lambda(lambda) => {
                self.enter_scope();
                let mut params = Vec::new();
                for param in &lambda.params {
                    let param_ty = self.resolve_type_ref(param.types.as_ref(), expr.span);
                    self.define_local(
                        &param.name,
                        SymbolKind::Variable,
                        param_ty.clone(),
                        expr.span,
                    );
                    params.push(param_ty);
                }
                let body_ty = self.check_expr(&lambda.body);
                self.exit_scope();
                let ret = lambda
                    .return_type
                    .as_ref()
                    .map(SemanticType::from_type_ref)
                    .unwrap_or(body_ty);

                if let Some(expected_ret) =
                    lambda.return_type.as_ref().map(SemanticType::from_type_ref)
                {
                    if !self.guarantees_value(&lambda.body) {
                        self.diagnostics.error(
                            format!(
                                "Lambda con retorno {} no garantiza valor en todos los caminos",
                                expected_ret
                            ),
                            expr.span,
                        );
                    }
                }
                SemanticType::Function(params, Box::new(ret))
            }
            KindExpr::New(new_expr) => {
                let arg_types = new_expr
                    .arguments
                    .iter()
                    .map(|arg| self.check_expr(arg))
                    .collect::<Vec<_>>();

                match &new_expr.type_info {
                    TypeRef::Custom(name) => self.check_constructor_call(name, &arg_types, expr.span),
                    TypeRef::Vector(inner) => {
                        // vector construction: expect at most one numeric size argument
                        if arg_types.len() > 1 {
                            self.diagnostics.error(
                                "Constructor de vector acepta a lo sumo un argumento de tamaño".to_string(),
                                expr.span,
                            );
                        }
                        if let Some(size_ty) = arg_types.get(0) {
                            self.expect_type(expr.span, size_ty, &SemanticType::Number, "tamaño de vector");
                        }
                        SemanticType::Vector(Box::new(SemanticType::from_type_ref(inner)))
                    }
                    other => {
                        // For other type refs (functions, etc.) fall back to unknown/custom handling
                        self.diagnostics.error(
                            format!("No se puede construir tipo: {:?}", other),
                            expr.span,
                        );
                        SemanticType::Unknown
                    }
                }
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
            KindExpr::Match(match_expr) => self.check_match_expr(match_expr, expr.span),
        };

        self.inferred_types.insert(expr.id, inferred.clone());
        inferred
    }

    /// Valida aridad y tipos de una llamada a funcion o functor.
    fn check_callable_signature(
        &mut self,
        params: &[SemanticType],
        ret: SemanticType,
        arg_types: &[SemanticType],
        span: Span,
    ) -> SemanticType {
        if params.len() != arg_types.len() {
            self.diagnostics.error(
                format!(
                    "Aridad invalida: se esperaban {} argumentos y llegaron {}",
                    params.len(),
                    arg_types.len()
                ),
                span,
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
                    span,
                );
            }
        }

        ret
    }

    /// Valida una construccion `new T(...)` o su alias `T(...)`.
    fn check_constructor_call(
        &mut self,
        type_name: &str,
        arg_types: &[SemanticType],
        span: Span,
    ) -> SemanticType {
        if let Some(symbol) = self.symbols.lookup(type_name) {
            if symbol.kind != SymbolKind::Type {
                self.diagnostics
                    .error(format!("{} no es un tipo construible", type_name), span);
            }
        } else {
            self.diagnostics
                .error(format!("Tipo no definido: {}", type_name), span);
        }

        if let Some(shape) = self.type_shapes.get(type_name).cloned() {
            if shape.ctor_params.len() != arg_types.len() {
                self.diagnostics.error(
                    format!(
                        "Constructor de {} espera {} argumentos y recibio {}",
                        type_name,
                        shape.ctor_params.len(),
                        arg_types.len()
                    ),
                    span,
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
                            type_name,
                            expected,
                            actual
                        ),
                        span,
                    );
                }
            }
        }

        SemanticType::Custom(type_name.to_string())
    }

    /// Valida una expresion `if`, incluyendo ramas `elif` y `else`.
    fn check_if_expr(&mut self, if_expr: &IfExpr, span: Span) -> SemanticType {
        //self.report_unreachable_if_branches(if_expr, span);

        let state_before_if = self.assigned_scopes.clone();
        self.assigned_scopes = state_before_if.clone();
        let cond_ty = self.check_expr(&if_expr.condition);
        self.expect_type(span, &cond_ty, &SemanticType::Boolean, "condicion de if");
        self.assigned_scopes = state_before_if.clone();

        let then_ty = if let Some((name, narrowed_type)) =
            self.extract_is_narrowing(&if_expr.condition, span)
        {
            self.enter_scope();
            self.define_local(&name, SymbolKind::Variable, narrowed_type, span);
            let ty = self.check_expr(&if_expr.then_branch);
            self.exit_scope();
            ty
        } else {
            self.check_expr(&if_expr.then_branch)
        };

        let then_state = self.assigned_scopes.clone();
        let mut branch_states = vec![then_state];

        let mut elif_types = Vec::new();
        for (elif_cond, elif_body) in &if_expr.elif_branches {
            self.assigned_scopes = state_before_if.clone();
            let elif_cond_ty = self.check_expr(elif_cond);
            self.expect_type(
                span,
                &elif_cond_ty,
                &SemanticType::Boolean,
                "condicion de elif",
            );
            self.assigned_scopes = state_before_if.clone();
            elif_types.push(self.check_expr(elif_body));
            branch_states.push(self.assigned_scopes.clone());
        }

        self.assigned_scopes = state_before_if.clone();
        let else_ty = self.check_expr(&if_expr.else_branch);
        branch_states.push(self.assigned_scopes.clone());
        self.assigned_scopes =
            self.merge_definite_assignment_states(&state_before_if, &branch_states);

        let mut combined_ty = then_ty;
        for elif_ty in elif_types {
            combined_ty = self.common_supertype(&combined_ty, &elif_ty);
        }
        self.common_supertype(&combined_ty, &else_ty)
    }

    /// Valida una expresion `match` y su exhaustividad.
    fn check_match_expr(&mut self, match_expr: &MatchExpr, span: Span) -> SemanticType {
        let scrutinee_ty = self.check_expr(&match_expr.expression);
        let state_before_cases = self.assigned_scopes.clone();
        let known_scrutinee_literal = match &match_expr.expression.kind {
            KindExpr::Literal(lit) => Some(&lit.value),
            _ => None,
        };
        let scrutinee_name = match &match_expr.expression.kind {
            KindExpr::Variable(var) => Some(var.name.as_str()),
            _ => None,
        };
        let mut merged = SemanticType::Unknown;
        let mut saw_true_case = false;
        let mut saw_false_case = false;
        let mut saw_default_case = false;
        let mut saw_catch_all_pattern = false;
        let mut saw_forced_match_case = false;
        let mut seen_literal_patterns = HashSet::new();
        let mut seen_type_coverages: Vec<SemanticType> = Vec::new();
        let mut case_states = Vec::new();

        for case in &match_expr.cases {
            let current_pattern_ty = self.pattern_coverage_type(&case.pattern, span);
            let shadowed_by_previous_type = current_pattern_ty.as_ref().is_some_and(|current| {
                seen_type_coverages
                    .iter()
                    .any(|previous| self.is_compatible_type(previous, current))
            });
            let known_literal_match = known_scrutinee_literal.and_then(|literal| {
                self.pattern_matches_known_literal(&case.pattern, literal, span)
            });
            let impossible_for_known_literal = matches!(known_literal_match, Some(false));

            if saw_default_case
                || saw_catch_all_pattern
                || saw_forced_match_case
                || shadowed_by_previous_type
                || impossible_for_known_literal
            {
                self.diagnostics.error(
                    "Caso inalcanzable: hay un case previo que ya cubre este patron",
                    span,
                );
            }

            self.assigned_scopes = state_before_cases.clone();
            self.enter_scope();
            self.check_pattern(&case.pattern, &scrutinee_ty, scrutinee_name, span);
            let case_ty = self.check_expr(&case.body);
            self.exit_scope();
            case_states.push(self.assigned_scopes.clone());

            match &case.pattern {
                Pattern::Literal(LiteralValue::Bool(true)) => {
                    if saw_true_case {
                        self.diagnostics
                            .error("Patron duplicado: case true repetido", span);
                    }
                    saw_true_case = true;
                }
                Pattern::Literal(LiteralValue::Bool(false)) => {
                    if saw_false_case {
                        self.diagnostics
                            .error("Patron duplicado: case false repetido", span);
                    }
                    saw_false_case = true;
                }
                Pattern::Identifier {
                    type_restriction: None,
                    ..
                } => {
                    saw_catch_all_pattern = true;
                }
                Pattern::Literal(lit) => {
                    let key = Self::literal_pattern_key(lit);
                    if !seen_literal_patterns.insert(key.clone()) {
                        self.diagnostics.error(
                            format!("Patron duplicado en match: case {} repetido", key),
                            span,
                        );
                    }
                }
                Pattern::Default => {
                    if saw_default_case {
                        self.diagnostics
                            .error("Patron duplicado: multiple default en match", span);
                    }
                    saw_default_case = true;
                }
                _ => {}
            }

            if let Some(coverage_ty) = current_pattern_ty {
                if self.is_compatible_type(&coverage_ty, &scrutinee_ty) {
                    saw_catch_all_pattern = true;
                }
                seen_type_coverages.push(coverage_ty);
            }

            if matches!(known_literal_match, Some(true)) {
                saw_forced_match_case = true;
            }

            if matches!(merged, SemanticType::Unknown) {
                merged = case_ty;
            } else {
                merged = self.common_supertype(&merged, &case_ty);
            }
        }

        if matches!(scrutinee_ty, SemanticType::Boolean)
            && !saw_default_case
            && !saw_forced_match_case
            && !(saw_true_case && saw_false_case)
        {
            self.diagnostics.error(
                "Match sobre Boolean no exhaustivo: faltan casos true/false o default",
                span,
            );
        }

        if !matches!(scrutinee_ty, SemanticType::Boolean | SemanticType::Unknown)
            && !saw_default_case
            && !saw_forced_match_case
        {
            self.diagnostics.error(
                format!("Match no exhaustivo para {}: falta default", scrutinee_ty),
                span,
            );
        }

        let exhaustive = saw_default_case
            || saw_forced_match_case
            || (matches!(scrutinee_ty, SemanticType::Boolean) && saw_true_case && saw_false_case);
        if exhaustive && !case_states.is_empty() {
            self.assigned_scopes =
                self.merge_definite_assignment_states(&state_before_cases, &case_states);
        } else {
            self.assigned_scopes = state_before_cases;
        }

        merged
    }

    /// Valida y enlaza un patron contra el tipo esperado.
    pub(super) fn check_pattern(
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
                self.mark_local_readonly(name, "binding de patron de match");

                if let Some(scrutinee_name) = scrutinee_name
                    && scrutinee_name != name
                {
                    self.define_local(scrutinee_name, SymbolKind::Variable, bound_type, span);
                    self.mark_local_readonly(scrutinee_name, "binding estrechado de match");
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

    /// Extrae un refinamiento de tipo a partir de una condicion `is`.
    pub(super) fn extract_is_narrowing(
        &mut self,
        condition: &Expr,
        span: Span,
    ) -> Option<(String, SemanticType)> {
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

    /// Valida una llamada a `base()` dentro de un metodo de tipo.
    pub(super) fn check_base_call(&mut self, call: &BaseCallExpr, span: Span) -> SemanticType {
        let Some(type_name) = self.current_type_context.clone() else {
            self.diagnostics
                .error("'base(...)' solo es valido dentro de metodos de tipo", span);
            for arg in &call.arguments {
                self.check_expr(arg);
            }
            return SemanticType::Unknown;
        };

        let Some(method_name) = self.current_method_context.clone() else {
            self.diagnostics.error(
                "'base(...)' solo es valido dentro del cuerpo de un metodo",
                span,
            );
            for arg in &call.arguments {
                self.check_expr(arg);
            }
            return SemanticType::Unknown;
        };

        let has_parent = self
            .type_shapes
            .get(&type_name)
            .and_then(|shape| shape.parent.as_ref())
            .is_some();
        if !has_parent {
            self.diagnostics.error(
                format!(
                    "No se puede usar base(...) en {}.{}: el tipo no tiene padre",
                    type_name, method_name
                ),
                span,
            );
            for arg in &call.arguments {
                self.check_expr(arg);
            }
            return SemanticType::Unknown;
        }

        let arg_types = call
            .arguments
            .iter()
            .map(|arg| self.check_expr(arg))
            .collect::<Vec<_>>();

        let Some(parent_signature) =
            self.lookup_member_type_in_parent_chain(&type_name, &method_name)
        else {
            self.diagnostics.error(
                format!(
                    "No existe implementacion base para {}.{} en la jerarquia padre",
                    type_name, method_name
                ),
                span,
            );
            return SemanticType::Unknown;
        };

        let SemanticType::Function(params, ret) = parent_signature else {
            self.diagnostics.error(
                format!(
                    "El miembro base {}.{} no es invocable como metodo",
                    type_name, method_name
                ),
                span,
            );
            return SemanticType::Unknown;
        };

        if params.len() != arg_types.len() {
            self.diagnostics.error(
                format!(
                    "Aridad invalida en base(...): se esperaban {} argumentos y llegaron {}",
                    params.len(),
                    arg_types.len()
                ),
                span,
            );
        }

        for (idx, (expected, actual)) in params.iter().zip(arg_types.iter()).enumerate() {
            if !self.is_compatible_type(expected, actual) {
                self.diagnostics.error(
                    format!(
                        "Argumento {} incompatible en base(...): se esperaba {}, se obtuvo {}",
                        idx + 1,
                        expected,
                        actual
                    ),
                    span,
                );
            }
        }

        *ret
    }

    /// Indica si un nodo es un destino valido de asignacion.
    pub(super) fn is_assignable_target(kind: &KindExpr) -> bool {
        matches!(
            kind,
            KindExpr::Variable(_) | KindExpr::MemberAccess(_) | KindExpr::Index(_)
        )
    }
}
