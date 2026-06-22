use std::collections::{HashMap, HashSet};

use crate::ast::{Expr, FunctionDecl, KindExpr, LiteralValue, Span, TypeDecl};
use crate::semantic::symbol_table::{Symbol, SymbolKind};
use crate::semantic::types::SemanticType;

use super::{SemanticAnalyzer, SymbolRequirements};

impl SemanticAnalyzer {
    /// Ejecuta la inferencia previa para funciones, tipos y metodos del programa.
    pub(super) fn infer_program_annotations(&mut self, program: &crate::ast::Program) {
        self.inferred_function_params.clear();
        self.inferred_function_returns.clear();
        self.inferred_type_params.clear();
        self.inferred_method_params.clear();
        self.inferred_method_returns.clear();

        for item in &program.items {
            match item {
                crate::ast::Item::Import(_) | crate::ast::Item::Export(_) => {}
                crate::ast::Item::Function(func) => {
                    let inferred = self.infer_param_types(func);
                    let resolved_params = func
                        .params
                        .iter()
                        .map(|param| {
                            param
                                .types
                                .as_ref()
                                .map(SemanticType::from_type_ref)
                                .unwrap_or_else(|| {
                                    inferred
                                        .get(&param.name)
                                        .cloned()
                                        .unwrap_or(SemanticType::Unknown)
                                })
                        })
                        .collect::<Vec<_>>();

                    let resolved_return = if func.return_type.is_some() {
                        func.return_type
                            .as_ref()
                            .map(SemanticType::from_type_ref)
                            .unwrap_or(SemanticType::Unknown)
                    } else {
                        // Use inferred param types as context so calls like `print(x)`
                        // can propagate `x`'s type into the inferred return.
                        self.infer_return_type_with_context(&func.body, func.body.span, &inferred)
                    };

                    self.inferred_function_params
                        .insert(func.name.clone(), inferred);
                    self.inferred_function_returns
                        .insert(func.name.clone(), resolved_return.clone());
                    let _ = self.symbols.update_type(
                        &func.name,
                        SemanticType::Function(resolved_params, Box::new(resolved_return)),
                    );
                }
                crate::ast::Item::Type(typ) => {
                    let inferred_type_params = self.infer_type_decl_params(typ);
                    self.inferred_type_params
                        .insert(typ.name.clone(), inferred_type_params.clone());

                    let prev_type_context = self.current_type_context.clone();
                    self.current_type_context = Some(typ.name.clone());

                    let mut method_map: HashMap<String, HashMap<String, SemanticType>> =
                        HashMap::new();
                    let mut method_return_map: HashMap<String, SemanticType> = HashMap::new();
                    for method in &typ.methods {
                        let inferred = self.infer_param_types(method);
                        method_map.insert(method.name.clone(), inferred.clone());
                        let ret = if method.return_type.is_some() {
                            method
                                .return_type
                                .as_ref()
                                .map(SemanticType::from_type_ref)
                                .unwrap_or(SemanticType::Unknown)
                        } else {
                            self.infer_return_type_with_context(
                                &method.body,
                                method.body.span,
                                &inferred,
                            )
                        };
                        method_return_map.insert(method.name.clone(), ret.clone());
                        if let Some(shape) = self.type_shapes.get_mut(&typ.name) {
                            let params = method
                                    .params
                                    .iter()
                                    .map(|param| {
                                        param
                                            .types
                                            .as_ref()
                                            .map(SemanticType::from_type_ref)
                                            .unwrap_or_else(|| {
                                                inferred
                                                    .get(&param.name)
                                                    .cloned()
                                                    .unwrap_or(SemanticType::Unknown)
                                            })
                                    })
                                    .collect::<Vec<_>>();
                                shape.methods.insert(method.name.clone(), SemanticType::Function(params, Box::new(ret)));
                        }
                    }
                    self.inferred_method_params
                        .insert(typ.name.clone(), method_map.clone());
                    self.inferred_method_returns
                        .insert(typ.name.clone(), method_return_map.clone());

                    self.current_type_context = prev_type_context;
                }
                _ => {}
            }
        }
    }

    /// Inferre tipos de parametros a partir de las restricciones recopiladas en el cuerpo.
    pub(super) fn infer_param_types(
        &mut self,
        func: &FunctionDecl,
    ) -> HashMap<String, SemanticType> {
        let inferable = func
            .params
            .iter()
            .filter(|param| param.types.is_none())
            .map(|param| param.name.clone())
            .collect::<HashSet<_>>();

        if inferable.is_empty() {
            return HashMap::new();
        }

        let mut requirements: HashMap<String, SymbolRequirements> = HashMap::new();
        let mut shadow_stack: Vec<HashSet<String>> = Vec::new();
        self.collect_inference_requirements(
            &func.body,
            &inferable,
            &mut shadow_stack,
            &mut requirements,
        );

        let mut inferred = HashMap::new();
        for name in inferable {
            let req = requirements.remove(&name).unwrap_or_default();
            let inferred_ty = self.synthesize_inferred_type(&name, req, func.body.span);
            inferred.insert(name, inferred_ty);
        }

        inferred
    }

    /// Inferre tipos de parametros de constructores de tipos declarados.
    pub(super) fn infer_type_decl_params(
        &mut self,
        typ: &TypeDecl,
    ) -> HashMap<String, SemanticType> {
        let inferable = typ
            .param
            .iter()
            .filter(|param| param.types.is_none())
            .map(|param| param.name.clone())
            .collect::<HashSet<_>>();

        if inferable.is_empty() {
            return HashMap::new();
        }

        let mut requirements: HashMap<String, SymbolRequirements> = HashMap::new();
        for field in &typ.fields {
            self.collect_inference_requirements(
                &field.initializer,
                &inferable,
                &mut Vec::new(),
                &mut requirements,
            );
        }
        for method in &typ.methods {
            let mut shadow_stack = vec![
                method
                    .params
                    .iter()
                    .map(|param| param.name.clone())
                    .collect(),
            ];
            self.collect_inference_requirements(
                &method.body,
                &inferable,
                &mut shadow_stack,
                &mut requirements,
            );
        }

        let mut inferred = HashMap::new();
        for name in inferable {
            let req = requirements.remove(&name).unwrap_or_default();
            inferred.insert(
                name.clone(),
                self.synthesize_inferred_type(&name, req, self.type_decl_span(typ)),
            );
        }

        inferred
    }

    /// Infiere el tipo de retorno a partir de las expresiones `return` encontradas.
    pub(super) fn infer_return_type_with_context(
        &self,
        body: &Expr,
        _span: Span,
        param_types: &HashMap<String, SemanticType>,
    ) -> SemanticType {
        let return_exprs = self.collect_return_expressions(body);

        if return_exprs.is_empty() {
            return SemanticType::Unknown;
        }

        let mut return_types = Vec::new();
        for ret_expr in return_exprs {
            return_types.push(self.infer_expr_type_hint_with_context(ret_expr, param_types));
        }

        self.merge_return_type_hints(return_types)
    }

    fn merge_return_type_hints(&self, return_types: Vec<SemanticType>) -> SemanticType {
        if return_types.is_empty() {
            return SemanticType::Unknown;
        }

        let first = return_types[0].clone();
        let all_same = return_types.iter().all(|t| match (t, &first) {
            (SemanticType::Number, SemanticType::Number) => true,
            (SemanticType::String, SemanticType::String) => true,
            (SemanticType::Boolean, SemanticType::Boolean) => true,
            (SemanticType::Vector(_), SemanticType::Vector(_)) => true,
            (SemanticType::Unknown, _) => true,
            (_, SemanticType::Unknown) => true,
            _ => false,
        });

        if all_same {
            first
        } else {
            SemanticType::Unknown
        }
    }

    fn infer_expr_type_hint_with_context(
        &self,
        expr: &Expr,
        param_types: &HashMap<String, SemanticType>,
    ) -> SemanticType {
        match &expr.kind {
            KindExpr::Literal(lit) => match lit.value {
                LiteralValue::Number(_) => SemanticType::Number,
                LiteralValue::String(_) => SemanticType::String,
                LiteralValue::Bool(_) => SemanticType::Boolean,
            },
            KindExpr::Variable(var) => {
                if var.name == "self" {
                    self.current_type_context
                        .as_ref()
                        .map(|name| SemanticType::Custom(name.clone()))
                        .unwrap_or(SemanticType::Unknown)
                } else {
                    param_types
                        .get(&var.name)
                        .cloned()
                        .or_else(|| self.symbols.lookup(&var.name).map(|sym| sym.typ.clone()))
                        .unwrap_or(SemanticType::Unknown)
                }
            }
            KindExpr::MemberAccess(member) => {
                let object_ty = self.infer_expr_type_hint_with_context(&member.object, param_types);
                match object_ty {
                    SemanticType::Custom(type_name) => self
                        .lookup_member_type(&type_name, &member.field)
                        .unwrap_or(SemanticType::Unknown),
                    SemanticType::Vector(inner) => match member.field.as_str() {
                        "size" => {
                            SemanticType::Function(Vec::new(), Box::new(SemanticType::Number))
                        }
                        "next" => {
                            SemanticType::Function(Vec::new(), Box::new(SemanticType::Boolean))
                        }
                        "current" => SemanticType::Function(Vec::new(), Box::new(*inner)),
                        _ => SemanticType::Unknown,
                    },
                    SemanticType::Function(_, _) if member.field == "invoke" => object_ty,
                    _ => SemanticType::Unknown,
                }
            }
            KindExpr::Call(call) => {
                if let KindExpr::Variable(var) = &call.callee.kind
                    && var.name == "print"
                {
                    return call
                        .arguments
                        .first()
                        .map(|arg| self.infer_expr_type_hint_with_context(arg, param_types))
                        .unwrap_or(SemanticType::Unknown);
                }

                match self.infer_expr_type_hint_with_context(&call.callee, param_types) {
                    SemanticType::Function(_, ret) => *ret,
                    SemanticType::Custom(type_name) => {
                        if let Some(SemanticType::Function(_, ret)) =
                            self.lookup_functor_invoke_type(&type_name)
                        {
                            *ret
                        } else {
                            SemanticType::Unknown
                        }
                    }
                    _ => SemanticType::Unknown,
                }
            }
            KindExpr::Binary(bin) => {
                use crate::ast::BinaryOperator::*;

                match bin.operator {
                    Add | Sub | Mul | Div | Pow | Mod => SemanticType::Number,
                    And | Or => SemanticType::Boolean,
                    Concat | FullConcat => SemanticType::String,
                    Equal | NotEqual | Less | Greater | LessEqual | GreaterEqual => {
                        SemanticType::Boolean
                    }
                }
            }
            KindExpr::Unary(unary) => match unary.operator {
                crate::ast::UnaryOperator::Negate => SemanticType::Number,
                crate::ast::UnaryOperator::Not => SemanticType::Boolean,
            },
            KindExpr::Assign(assign) => {
                self.infer_expr_type_hint_with_context(&assign.target, param_types)
            }
            KindExpr::Block(block) => block
                .expressions
                .last()
                .map(|expr| self.infer_expr_type_hint_with_context(expr, param_types))
                .unwrap_or(SemanticType::Boolean),
            KindExpr::Array(array) => {
                let element_ty = array
                    .elements
                    .first()
                    .map(|expr| self.infer_expr_type_hint_with_context(expr, param_types))
                    .unwrap_or(SemanticType::Unknown);
                SemanticType::Vector(Box::new(element_ty))
            }
            KindExpr::If(if_expr) => {
                let then_ty =
                    self.infer_expr_type_hint_with_context(&if_expr.then_branch, param_types);
                let else_ty =
                    self.infer_expr_type_hint_with_context(&if_expr.else_branch, param_types);
                self.common_supertype(&then_ty, &else_ty)
            }
            KindExpr::Let(let_expr) => {
                self.infer_expr_type_hint_with_context(&let_expr.body, param_types)
            }
            KindExpr::Match(match_expr) => {
                let mut ty = SemanticType::Unknown;
                for case in &match_expr.cases {
                    let case_ty = self.infer_expr_type_hint_with_context(&case.body, param_types);
                    ty = if matches!(ty, SemanticType::Unknown) {
                        case_ty
                    } else {
                        self.common_supertype(&ty, &case_ty)
                    };
                }
                ty
            }
            _ => SemanticType::Unknown,
        }
    }

    /// Recolecta las expresiones retornadas dentro de un cuerpo.
    pub(super) fn collect_return_expressions<'a>(&self, expr: &'a Expr) -> Vec<&'a Expr> {
        let mut returns = Vec::new();
        self.collect_returns_helper(expr, &mut returns);
        returns
    }

    /// Recorre recursivamente un arbol de expresion buscando retornos.
    pub(super) fn collect_returns_helper<'a>(&self, expr: &'a Expr, returns: &mut Vec<&'a Expr>) {
        match &expr.kind {
            KindExpr::Block(block) => {
                if block.expressions.is_empty() {
                    returns.push(expr);
                } else {
                    self.collect_returns_helper(
                        &block.expressions[block.expressions.len() - 1],
                        returns,
                    );
                }
            }
            KindExpr::If(if_expr) => {
                self.collect_returns_helper(&if_expr.then_branch, returns);
                for (_, body) in &if_expr.elif_branches {
                    self.collect_returns_helper(body, returns);
                }
                self.collect_returns_helper(&if_expr.else_branch, returns);
            }
            KindExpr::Let(let_expr) => {
                self.collect_returns_helper(&let_expr.body, returns);
            }
            KindExpr::Match(match_expr) => {
                for case in &match_expr.cases {
                    self.collect_returns_helper(&case.body, returns);
                }
            }
            _ => {
                returns.push(expr);
            }
        }
    }

    /// Recolecta restricciones de inferencia desde una expresion y sus subexpresiones.
    pub(super) fn collect_inference_requirements(
        &mut self,
        expr: &Expr,
        inferable: &HashSet<String>,
        shadow_stack: &mut Vec<HashSet<String>>,
        requirements: &mut HashMap<String, SymbolRequirements>,
    ) {
        let is_shadowed = |name: &str, stack: &Vec<HashSet<String>>| -> bool {
            stack.iter().rev().any(|scope| scope.contains(name))
        };

        let record_concrete = |requirements: &mut HashMap<String, SymbolRequirements>,
                               name: &str,
                               ty: SemanticType,
                               diagnostics: &mut crate::diagnostics::DiagnosticCollector,
                               span: Span| {
            let entry = requirements.entry(name.to_string()).or_default();
            if let Some(existing) = &entry.concrete {
                if existing != &ty
                    && !existing.is_assignable_from(&ty)
                    && !ty.is_assignable_from(existing)
                {
                    diagnostics.error(
                        format!(
                            "La inferencia del simbolo {} es incompatible entre {} y {}",
                            name, existing, ty
                        ),
                        span,
                    );
                }
            } else {
                entry.concrete = Some(ty);
            }
        };

        match &expr.kind {
            KindExpr::Literal(_) => {}
            KindExpr::Variable(var) => {
                if inferable.contains(&var.name) && !is_shadowed(&var.name, shadow_stack) {
                    requirements.entry(var.name.clone()).or_default();
                }
            }
            KindExpr::Binary(bin) => {
                self.collect_inference_requirements(
                    &bin.left,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                self.collect_inference_requirements(
                    &bin.right,
                    inferable,
                    shadow_stack,
                    requirements,
                );

                let left_name = match &bin.left.kind {
                    KindExpr::Variable(var)
                        if inferable.contains(&var.name)
                            && !is_shadowed(&var.name, shadow_stack) =>
                    {
                        Some(var.name.clone())
                    }
                    _ => None,
                };
                let right_name = match &bin.right.kind {
                    KindExpr::Variable(var)
                        if inferable.contains(&var.name)
                            && !is_shadowed(&var.name, shadow_stack) =>
                    {
                        Some(var.name.clone())
                    }
                    _ => None,
                };

                use crate::ast::BinaryOperator::*;
                match bin.operator {
                    Add | Sub | Mul | Div | Pow | Mod => {
                        if let Some(name) = left_name {
                            record_concrete(
                                requirements,
                                &name,
                                SemanticType::Number,
                                &mut self.diagnostics,
                                expr.span,
                            );
                        }
                        if let Some(name) = right_name {
                            record_concrete(
                                requirements,
                                &name,
                                SemanticType::Number,
                                &mut self.diagnostics,
                                expr.span,
                            );
                        }
                    }
                    And | Or => {
                        if let Some(name) = left_name {
                            record_concrete(
                                requirements,
                                &name,
                                SemanticType::Boolean,
                                &mut self.diagnostics,
                                expr.span,
                            );
                        }
                        if let Some(name) = right_name {
                            record_concrete(
                                requirements,
                                &name,
                                SemanticType::Boolean,
                                &mut self.diagnostics,
                                expr.span,
                            );
                        }
                    }
                    Concat | FullConcat => {
                        if let Some(name) = left_name {
                            record_concrete(
                                requirements,
                                &name,
                                SemanticType::String,
                                &mut self.diagnostics,
                                expr.span,
                            );
                        }
                        if let Some(name) = right_name {
                            record_concrete(
                                requirements,
                                &name,
                                SemanticType::String,
                                &mut self.diagnostics,
                                expr.span,
                            );
                        }
                    }
                    Less | Greater | LessEqual | GreaterEqual => {
                        if let Some(name) = left_name {
                            record_concrete(
                                requirements,
                                &name,
                                SemanticType::Number,
                                &mut self.diagnostics,
                                expr.span,
                            );
                        }
                        if let Some(name) = right_name {
                            record_concrete(
                                requirements,
                                &name,
                                SemanticType::Number,
                                &mut self.diagnostics,
                                expr.span,
                            );
                        }
                    }
                    Equal | NotEqual => {}
                }
            }
            KindExpr::Unary(unary) => {
                self.collect_inference_requirements(
                    &unary.right,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                if let KindExpr::Variable(var) = &unary.right.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    use crate::ast::UnaryOperator::*;
                    let ty = match unary.operator {
                        Negate => SemanticType::Number,
                        Not => SemanticType::Boolean,
                    };
                    record_concrete(
                        requirements,
                        &var.name,
                        ty,
                        &mut self.diagnostics,
                        expr.span,
                    );
                }
            }
            KindExpr::Call(call) => {
                self.collect_inference_requirements(
                    &call.callee,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                for arg in &call.arguments {
                    self.collect_inference_requirements(arg, inferable, shadow_stack, requirements);
                }

                if let KindExpr::MemberAccess(member) = &call.callee.kind
                    && let KindExpr::Variable(var) = &member.object.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    requirements
                        .entry(var.name.clone())
                        .or_default()
                        .methods
                        .insert(member.field.clone(), call.arguments.len());
                } else if let KindExpr::Variable(var) = &call.callee.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    requirements
                        .entry(var.name.clone())
                        .or_default()
                        .requires_function = Some(call.arguments.len());
                }

                // Propagate known callee parameter types to inferable arguments.
                if let KindExpr::Variable(callee_var) = &call.callee.kind {
                    if let Some(SemanticType::Function(param_tys, _)) = self
                        .symbols
                        .lookup(&callee_var.name)
                        .map(|s| s.typ.clone())
                    {
                        for (arg, param_ty) in call.arguments.iter().zip(param_tys.iter()) {
                            if let KindExpr::Variable(arg_var) = &arg.kind
                                && inferable.contains(&arg_var.name)
                                && !is_shadowed(&arg_var.name, shadow_stack)
                                && !matches!(param_ty, SemanticType::Unknown)
                            {
                                record_concrete(
                                    requirements,
                                    &arg_var.name,
                                    param_ty.clone(),
                                    &mut self.diagnostics,
                                    expr.span,
                                );
                            }
                        }
                    }
                }
            }
            KindExpr::BaseCall(base_call) => {
                for arg in &base_call.arguments {
                    self.collect_inference_requirements(arg, inferable, shadow_stack, requirements);
                }
            }
            KindExpr::MacroCall(macr) => {
                for arg in &macr.arguments {
                    self.collect_inference_requirements(
                        &arg.value,
                        inferable,
                        shadow_stack,
                        requirements,
                    );
                }
                if let Some(action) = &macr.action {
                    self.collect_inference_requirements(
                        action,
                        inferable,
                        shadow_stack,
                        requirements,
                    );
                }
            }
            KindExpr::Let(let_expr) => {
                for binding in &let_expr.bindings {
                    self.collect_inference_requirements(
                        &binding.initializer,
                        inferable,
                        shadow_stack,
                        requirements,
                    );
                    let mut scope = HashSet::new();
                    scope.insert(binding.name.clone());
                    shadow_stack.push(scope);
                }
                self.collect_inference_requirements(
                    &let_expr.body,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                for _ in &let_expr.bindings {
                    shadow_stack.pop();
                }
            }
            KindExpr::Block(block) => {
                for expr in &block.expressions {
                    self.collect_inference_requirements(
                        expr,
                        inferable,
                        shadow_stack,
                        requirements,
                    );
                }
            }
            KindExpr::If(if_expr) => {
                self.collect_inference_requirements(
                    &if_expr.condition,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                self.collect_inference_requirements(
                    &if_expr.then_branch,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                for (cond, body) in &if_expr.elif_branches {
                    self.collect_inference_requirements(
                        cond,
                        inferable,
                        shadow_stack,
                        requirements,
                    );
                    self.collect_inference_requirements(
                        body,
                        inferable,
                        shadow_stack,
                        requirements,
                    );
                }
                self.collect_inference_requirements(
                    &if_expr.else_branch,
                    inferable,
                    shadow_stack,
                    requirements,
                );
            }
            KindExpr::While(while_expr) => {
                self.collect_inference_requirements(
                    &while_expr.condition,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                self.collect_inference_requirements(
                    &while_expr.body,
                    inferable,
                    shadow_stack,
                    requirements,
                );
            }
            KindExpr::For(for_expr) => {
                self.collect_inference_requirements(
                    &for_expr.iterable,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                let mut scope = HashSet::new();
                scope.insert(for_expr.variable.clone());
                shadow_stack.push(scope);
                self.collect_inference_requirements(
                    &for_expr.body,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                shadow_stack.pop();
            }
            KindExpr::Assign(assign) => {
                self.collect_inference_requirements(
                    &assign.target,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                self.collect_inference_requirements(
                    &assign.value,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                if let KindExpr::Variable(var) = &assign.target.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    let value_ty = match &assign.value.kind {
                        KindExpr::Literal(lit) => match lit.value {
                            LiteralValue::Number(_) => SemanticType::Number,
                            LiteralValue::String(_) => SemanticType::String,
                            LiteralValue::Bool(_) => SemanticType::Boolean,
                        },
                        _ => SemanticType::Unknown,
                    };
                    record_concrete(
                        requirements,
                        &var.name,
                        value_ty,
                        &mut self.diagnostics,
                        expr.span,
                    );
                } else if let KindExpr::MemberAccess(member) = &assign.target.kind
                    && let KindExpr::Variable(object) = &member.object.kind
                    && object.name == "self"
                    && let Some(type_name) = self.current_type_context.as_deref()
                    && let Some(target_ty) = self.lookup_member_type(type_name, &member.field)
                    && !matches!(target_ty, SemanticType::Unknown)
                    && let KindExpr::Variable(var) = &assign.value.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    record_concrete(
                        requirements,
                        &var.name,
                        target_ty,
                        &mut self.diagnostics,
                        expr.span,
                    );
                }
            }
            KindExpr::MemberAccess(member) => {
                self.collect_inference_requirements(
                    &member.object,
                    inferable,
                    shadow_stack,
                    requirements,
                );
            }
            KindExpr::Index(index) => {
                self.collect_inference_requirements(
                    &index.object,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                self.collect_inference_requirements(
                    &index.index,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                if let KindExpr::Variable(var) = &index.object.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    requirements
                        .entry(var.name.clone())
                        .or_default()
                        .requires_vector = true;
                }
            }
            KindExpr::Array(array) => {
                for element in &array.elements {
                    self.collect_inference_requirements(
                        element,
                        inferable,
                        shadow_stack,
                        requirements,
                    );
                }
            }
            KindExpr::ArrayComprehension(comp) => {
                self.collect_inference_requirements(
                    &comp.iterable,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                let mut scope = HashSet::new();
                scope.insert(comp.variable.clone());
                shadow_stack.push(scope);
                self.collect_inference_requirements(
                    &comp.element,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                shadow_stack.pop();
            }
            KindExpr::Lambda(lambda) => {
                for param in &lambda.params {
                    let mut scope = HashSet::new();
                    scope.insert(param.name.clone());
                    shadow_stack.push(scope);
                }
                self.collect_inference_requirements(
                    &lambda.body,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                for _ in &lambda.params {
                    shadow_stack.pop();
                }
            }
            KindExpr::New(new_expr) => {
                for arg in &new_expr.arguments {
                    self.collect_inference_requirements(arg, inferable, shadow_stack, requirements);
                }
            }
            KindExpr::Is(is_expr) => {
                self.collect_inference_requirements(
                    &is_expr.expression,
                    inferable,
                    shadow_stack,
                    requirements,
                );
            }
            KindExpr::As(as_expr) => {
                self.collect_inference_requirements(
                    &as_expr.expression,
                    inferable,
                    shadow_stack,
                    requirements,
                );
            }
            KindExpr::Match(match_expr) => {
                self.collect_inference_requirements(
                    &match_expr.expression,
                    inferable,
                    shadow_stack,
                    requirements,
                );
                for case in &match_expr.cases {
                    self.collect_inference_requirements(
                        &case.body,
                        inferable,
                        shadow_stack,
                        requirements,
                    );
                }
            }
        }
    }

    /// Convierte restricciones en un tipo sintetico concreto o en un protocolo generado.
    pub(super) fn synthesize_inferred_type(
        &mut self,
        symbol_name: &str,
        req: SymbolRequirements,
        span: Span,
    ) -> SemanticType {
        if req.requires_vector {
            if let Some(concrete) = req.concrete.clone() {
                if !matches!(concrete, SemanticType::Vector(_)) {
                    self.diagnostics.error(
                        format!(
                            "La inferencia de {} requiere vector, pero se obtuvo {}",
                            symbol_name, concrete
                        ),
                        span,
                    );
                }
                return concrete;
            }
            return SemanticType::Vector(Box::new(SemanticType::Unknown));
        }

        if let Some(arity) = req.requires_function {
            let inferred = SemanticType::Function(
                vec![SemanticType::Unknown; arity],
                Box::new(SemanticType::Unknown),
            );
            if let Some(concrete) = req.concrete.clone() {
                if !matches!(concrete, SemanticType::Function(_, _)) {
                    self.diagnostics.error(
                        format!(
                            "La inferencia de {} requiere funcion, pero se obtuvo {}",
                            symbol_name, concrete
                        ),
                        span,
                    );
                }
                return concrete;
            }
            return inferred;
        }

        if !req.methods.is_empty() {
            let protocol_name = self.next_synthetic_protocol_name();
            let mut methods = HashMap::new();
            for (method_name, arity) in req.methods {
                methods.insert(
                    method_name,
                    SemanticType::Function(
                        vec![SemanticType::Unknown; arity],
                        Box::new(SemanticType::Unknown),
                    ),
                );
            }

            self.symbols.define(Symbol {
                name: protocol_name.clone(),
                kind: SymbolKind::Protocol,
                typ: SemanticType::Custom(protocol_name.clone()),
            });
            self.protocol_shapes.insert(
                protocol_name.clone(),
                super::ProtocolShape {
                    methods,
                    parent: None,
                },
            );

            if let Some(concrete) = req.concrete.clone() {
                match &concrete {
                    SemanticType::Custom(type_name) => {
                        if self.type_conforms_to_protocol(type_name, &protocol_name) {
                            return concrete;
                        }
                        self.diagnostics.error(
                            format!(
                                "El simbolo {} no cumple el protocolo sintetico {}",
                                symbol_name, protocol_name
                            ),
                            span,
                        );
                    }
                    _ => {
                        self.diagnostics.error(
                            format!(
                                "El simbolo {} no puede combinar usos estructurales con el tipo {}",
                                symbol_name, concrete
                            ),
                            span,
                        );
                    }
                }
            }

            return SemanticType::Custom(protocol_name);
        }

        req.concrete.unwrap_or(SemanticType::Unknown)
    }

    /// Genera el siguiente nombre unico para un protocolo sintetico.
    pub(super) fn next_synthetic_protocol_name(&mut self) -> String {
        self.synthetic_protocol_counter += 1;
        format!("_P{}", self.synthetic_protocol_counter)
    }
}
