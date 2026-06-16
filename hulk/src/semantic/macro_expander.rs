use std::collections::HashMap;

use crate::ast::*;
use crate::diagnostics::{Diagnostic, DiagnosticCollector};

pub fn expand_program(program: Program) -> Result<Program, Vec<Diagnostic>> {
    let macros = collect_macros(&program);
    let mut expander = MacroExpander::new(macros);
    let items = program
        .items
        .into_iter()
        .map(|item| expander.expand_item(item))
        .collect::<Vec<_>>();

    if expander.diagnostics.has_errors() {
        Err(expander.diagnostics.into_vec())
    } else {
        Ok(Program { items })
    }
}

fn collect_macros(program: &Program) -> HashMap<String, MacroDecl> {
    let mut macros = HashMap::new();
    for item in &program.items {
        if let Item::Macro(macr) = item {
            macros.insert(macr.name.clone(), macr.clone());
        }
    }
    macros
}

struct MacroExpander {
    macros: HashMap<String, MacroDecl>,
    diagnostics: DiagnosticCollector,
    fresh_counter: usize,
}

impl MacroExpander {
    fn new(macros: HashMap<String, MacroDecl>) -> Self {
        Self {
            macros,
            diagnostics: DiagnosticCollector::new(),
            fresh_counter: 0,
        }
    }

    fn fresh_name(&mut self, base: &str) -> String {
        self.fresh_counter += 1;
        format!("__macro_{}_{}", base, self.fresh_counter)
    }

    fn expand_item(&mut self, item: Item) -> Item {
        match item {
            Item::Import(imp) => Item::Import(imp),
            Item::Export(exp) => Item::Export(exp),
            Item::Function(mut func) => {
                func.body =
                    self.expand_expr(&func.body, &HashMap::new(), &[], &mut Vec::new(), false);
                Item::Function(func)
            }
            Item::Type(mut typ) => {
                for field in &mut typ.fields {
                    field.initializer = self.expand_expr(
                        &field.initializer,
                        &HashMap::new(),
                        &[],
                        &mut Vec::new(),
                        false,
                    );
                }
                for method in &mut typ.methods {
                    method.body = self.expand_expr(
                        &method.body,
                        &HashMap::new(),
                        &[],
                        &mut Vec::new(),
                        false,
                    );
                }
                Item::Type(typ)
            }
            Item::Macro(mut macr) => {
                macr.body = self.expand_expr(
                    &macr.body,
                    &HashMap::new(),
                    &[],
                    &mut Vec::new(),
                    false,
                );
                Item::Macro(macr)
            }
            Item::Protocol(proto) => Item::Protocol(proto),
            Item::GlobalExpr(expr) => Item::GlobalExpr(self.expand_expr(
                &expr,
                &HashMap::new(),
                &[],
                &mut Vec::new(),
                false,
            )),
        }
    }

    fn expand_expr(
        &mut self,
        expr: &Expr,
        substitutions: &HashMap<String, Expr>,
        renames: &[HashMap<String, String>],
        expansion_stack: &mut Vec<String>,
        sanitize_locals: bool,
    ) -> Expr {
        match &expr.kind {
            KindExpr::Literal(_) => expr.clone(),
            KindExpr::Variable(var) => {
                if let Some(replacement) = substitutions.get(&var.name) {
                    return replacement.clone();
                }

                if let Some(new_name) = self.lookup_rename(renames, &var.name) {
                    mk_expr(KindExpr::Variable(VariableExpr { name: new_name }))
                } else {
                    expr.clone()
                }
            }
            KindExpr::Binary(bin) => mk_expr(KindExpr::Binary(BinaryExpr {
                left: Box::new(self.expand_expr(
                    &bin.left,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                operator: bin.operator.clone(),
                right: Box::new(self.expand_expr(
                    &bin.right,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
            })),
            KindExpr::Unary(unary) => mk_expr(KindExpr::Unary(UnaryExpr {
                operator: unary.operator.clone(),
                right: Box::new(self.expand_expr(
                    &unary.right,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
            })),
            KindExpr::Call(call) => {
                let callee = self.expand_expr(
                    &call.callee,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                );
                let arguments = call
                    .arguments
                    .iter()
                    .map(|arg| {
                        self.expand_expr(
                            arg,
                            substitutions,
                            renames,
                            expansion_stack,
                            sanitize_locals,
                        )
                    })
                    .collect::<Vec<_>>();

                if let KindExpr::Variable(var) = &callee.kind {
                    if self.macros.contains_key(&var.name) {
                        let macro_call = MacroCallExpr {
                            name: var.name.clone(),
                            arguments: arguments
                                .into_iter()
                                .map(|value| MacroCallArg {
                                    kind: MacroParamKind::Normal,
                                    value,
                                })
                                .collect(),
                            action: None,
                        };
                        return self.expand_macro_call(
                            expr.span,
                            &macro_call,
                            substitutions,
                            renames,
                            expansion_stack,
                            sanitize_locals,
                        );
                    }
                }

                mk_expr(KindExpr::Call(CallExpr {
                    callee: Box::new(callee),
                    arguments,
                }))
            }
            KindExpr::BaseCall(call) => mk_expr(KindExpr::BaseCall(BaseCallExpr {
                arguments: call
                    .arguments
                    .iter()
                    .map(|arg| {
                        self.expand_expr(
                            arg,
                            substitutions,
                            renames,
                            expansion_stack,
                            sanitize_locals,
                        )
                    })
                    .collect(),
            })),
            KindExpr::MacroCall(call) => self.expand_macro_call(
                expr.span,
                call,
                substitutions,
                renames,
                expansion_stack,
                sanitize_locals,
            ),
            KindExpr::Let(let_expr) => {
                let mut bindings = Vec::new();
                let mut local_renames = renames.to_vec();

                for binding in &let_expr.bindings {
                    let initializer = self.expand_expr(
                        &binding.initializer,
                        substitutions,
                        &local_renames,
                        expansion_stack,
                        sanitize_locals,
                    );
                    let mut new_binding = binding.clone();
                    let fresh_name = if sanitize_locals {
                        self.fresh_name(&binding.name)
                    } else {
                        binding.name.clone()
                    };
                    new_binding.name = fresh_name.clone();
                    new_binding.initializer = initializer;
                    bindings.push(new_binding);

                    if sanitize_locals {
                        let mut scope = HashMap::new();
                        scope.insert(binding.name.clone(), fresh_name);
                        local_renames.push(scope);
                    }
                }

                let body = self.expand_expr(
                    &let_expr.body,
                    substitutions,
                    &local_renames,
                    expansion_stack,
                    sanitize_locals,
                );
                mk_expr(KindExpr::Let(LetExpr {
                    bindings,
                    body: Box::new(body),
                }))
            }
            KindExpr::Block(block) => mk_expr(KindExpr::Block(BlockExpr {
                expressions: block
                    .expressions
                    .iter()
                    .map(|sub| {
                        self.expand_expr(
                            sub,
                            substitutions,
                            renames,
                            expansion_stack,
                            sanitize_locals,
                        )
                    })
                    .collect(),
            })),
            KindExpr::If(if_expr) => mk_expr(KindExpr::If(IfExpr {
                condition: Box::new(self.expand_expr(
                    &if_expr.condition,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                then_branch: Box::new(self.expand_expr(
                    &if_expr.then_branch,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                elif_branches: if_expr
                    .elif_branches
                    .iter()
                    .map(|(cond, body)| {
                        (
                            self.expand_expr(
                                cond,
                                substitutions,
                                renames,
                                expansion_stack,
                                sanitize_locals,
                            ),
                            self.expand_expr(
                                body,
                                substitutions,
                                renames,
                                expansion_stack,
                                sanitize_locals,
                            ),
                        )
                    })
                    .collect(),
                else_branch: Box::new(self.expand_expr(
                    &if_expr.else_branch,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
            })),
            KindExpr::While(while_expr) => mk_expr(KindExpr::While(WhileExpr {
                condition: Box::new(self.expand_expr(
                    &while_expr.condition,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                body: Box::new(self.expand_expr(
                    &while_expr.body,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
            })),
            KindExpr::For(for_expr) => {
                let iterable = self.expand_expr(
                    &for_expr.iterable,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                );
                let (variable, local_renames) = if sanitize_locals {
                    let fresh_name = self.fresh_name(&for_expr.variable);
                    let mut local_renames = renames.to_vec();
                    let mut scope = HashMap::new();
                    scope.insert(for_expr.variable.clone(), fresh_name.clone());
                    local_renames.push(scope);
                    (fresh_name, local_renames)
                } else {
                    (for_expr.variable.clone(), renames.to_vec())
                };

                mk_expr(KindExpr::For(ForExpr {
                    variable,
                    iterable: Box::new(iterable),
                    body: Box::new(self.expand_expr(
                        &for_expr.body,
                        substitutions,
                        &local_renames,
                        expansion_stack,
                        sanitize_locals,
                    )),
                }))
            }
            KindExpr::Assign(assign) => mk_expr(KindExpr::Assign(AssignExpr {
                target: Box::new(self.expand_expr(
                    &assign.target,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                value: Box::new(self.expand_expr(
                    &assign.value,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
            })),
            KindExpr::MemberAccess(member) => mk_expr(KindExpr::MemberAccess(MemberAccessExpr {
                object: Box::new(self.expand_expr(
                    &member.object,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                field: member.field.clone(),
            })),
            KindExpr::Index(index) => mk_expr(KindExpr::Index(IndexExpr {
                object: Box::new(self.expand_expr(
                    &index.object,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                index: Box::new(self.expand_expr(
                    &index.index,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
            })),
            KindExpr::Array(array) => mk_expr(KindExpr::Array(ArrayExpr {
                elements: array
                    .elements
                    .iter()
                    .map(|element| {
                        self.expand_expr(
                            element,
                            substitutions,
                            renames,
                            expansion_stack,
                            sanitize_locals,
                        )
                    })
                    .collect(),
            })),
            KindExpr::ArrayComprehension(comp) => {
                let iterable = self.expand_expr(
                    &comp.iterable,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                );
                let (variable, local_renames) = if sanitize_locals {
                    let fresh_name = self.fresh_name(&comp.variable);
                    let mut local_renames = renames.to_vec();
                    let mut scope = HashMap::new();
                    scope.insert(comp.variable.clone(), fresh_name.clone());
                    local_renames.push(scope);
                    (fresh_name, local_renames)
                } else {
                    (comp.variable.clone(), renames.to_vec())
                };

                mk_expr(KindExpr::ArrayComprehension(ArrayComprehensionExpr {
                    element: Box::new(self.expand_expr(
                        &comp.element,
                        substitutions,
                        &local_renames,
                        expansion_stack,
                        sanitize_locals,
                    )),
                    variable,
                    iterable: Box::new(iterable),
                }))
            }
            KindExpr::Lambda(lambda) => {
                let mut local_renames = renames.to_vec();
                let mut params = Vec::new();

                if sanitize_locals {
                    let mut scope = HashMap::new();
                    for param in &lambda.params {
                        let fresh_name = self.fresh_name(&param.name);
                        scope.insert(param.name.clone(), fresh_name.clone());
                        let mut new_param = param.clone();
                        new_param.name = fresh_name;
                        params.push(new_param);
                    }
                    local_renames.push(scope);
                } else {
                    params = lambda.params.clone();
                }

                mk_expr(KindExpr::Lambda(LambdaExpr {
                    params,
                    return_type: lambda.return_type.clone(),
                    body: Box::new(self.expand_expr(
                        &lambda.body,
                        substitutions,
                        &local_renames,
                        expansion_stack,
                        sanitize_locals,
                    )),
                }))
            }
            KindExpr::New(new_expr) => mk_expr(KindExpr::New(NewExpr {
                type_name: new_expr.type_name.clone(),
                arguments: new_expr
                    .arguments
                    .iter()
                    .map(|arg| {
                        self.expand_expr(
                            arg,
                            substitutions,
                            renames,
                            expansion_stack,
                            sanitize_locals,
                        )
                    })
                    .collect(),
            })),
            KindExpr::Is(is_expr) => mk_expr(KindExpr::Is(IsExpr {
                expression: Box::new(self.expand_expr(
                    &is_expr.expression,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                type_info: is_expr.type_info.clone(),
            })),
            KindExpr::As(as_expr) => mk_expr(KindExpr::As(AsExpr {
                expression: Box::new(self.expand_expr(
                    &as_expr.expression,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                )),
                type_info: as_expr.type_info.clone(),
            })),
            KindExpr::Match(match_expr) => {
                let expression = self.expand_expr(
                    &match_expr.expression,
                    substitutions,
                    renames,
                    expansion_stack,
                    sanitize_locals,
                );
                let mut cases = Vec::new();

                for case in &match_expr.cases {
                    let (pattern, bindings) = self.expand_pattern(&case.pattern, sanitize_locals);
                    let mut local_renames = renames.to_vec();
                    if sanitize_locals && !bindings.is_empty() {
                        local_renames.push(bindings);
                    }
                    let body = self.expand_expr(
                        &case.body,
                        substitutions,
                        &local_renames,
                        expansion_stack,
                        sanitize_locals,
                    );
                    cases.push(MatchCase { pattern, body });
                }

                mk_expr(KindExpr::Match(MatchExpr {
                    expression: Box::new(expression),
                    cases,
                }))
            }
        }
    }

    fn expand_macro_call(
        &mut self,
        span: Span,
        call: &MacroCallExpr,
        substitutions: &HashMap<String, Expr>,
        renames: &[HashMap<String, String>],
        expansion_stack: &mut Vec<String>,
        sanitize_locals: bool,
    ) -> Expr {
        let Some(macr) = self.macros.get(&call.name).cloned() else {
            self.diagnostics
                .error(format!("Macro no definida: {}", call.name), span);
            return mk_expr(KindExpr::MacroCall(call.clone()));
        };

        if expansion_stack.iter().any(|name| name == &call.name) {
            self.diagnostics.error(
                format!("Expansión recursiva detectada en macro {}", call.name),
                span,
            );
            return mk_expr(KindExpr::MacroCall(call.clone()));
        }

        let mut arg_iter = call.arguments.iter();
        let mut call_substitutions = substitutions.clone();
        let mut block_param_seen = false;
        for param in &macr.params {
            match param.kind {
                MacroParamKind::Block => {
                    if block_param_seen {
                        self.diagnostics.error(
                            format!("La macro {} tiene mas de un parametro bloque", macr.name),
                            span,
                        );
                    }
                    block_param_seen = true;

                    let Some(action) = &call.action else {
                        self.diagnostics.error(
                            format!("La macro {} espera un bloque trailing", macr.name),
                            span,
                        );
                        continue;
                    };

                    call_substitutions.insert(
                        param.name.clone(),
                        self.expand_expr(
                            action,
                            substitutions,
                            renames,
                            expansion_stack,
                            sanitize_locals,
                        ),
                    );
                }
                MacroParamKind::Normal | MacroParamKind::Symbolic | MacroParamKind::Placeholder => {
                    let Some(arg) = arg_iter.next() else {
                        self.diagnostics.error(
                            format!("La macro {} espera mas argumentos", macr.name),
                            span,
                        );
                        continue;
                    };
                    if matches!(param.kind, MacroParamKind::Normal)
                        && !matches!(arg.kind, MacroParamKind::Normal)
                    {
                        self.diagnostics.error(
                            format!(
                                "La macro {} espera un argumento normal para {}",
                                macr.name, param.name
                            ),
                            span,
                        );
                    }
                    if matches!(param.kind, MacroParamKind::Symbolic)
                        && !matches!(arg.kind, MacroParamKind::Symbolic)
                    {
                        self.diagnostics.error(
                            format!(
                                "La macro {} espera un argumento simbolico para {}",
                                macr.name, param.name
                            ),
                            span,
                        );
                    }

                    let replacement = match param.kind {
                        MacroParamKind::Placeholder => match &arg.value.kind {
                            KindExpr::Variable(var) => mk_expr(KindExpr::Variable(VariableExpr {
                                name: var.name.clone(),
                            })),
                            _ => {
                                self.diagnostics.error(
                                    format!(
                                        "El parametro {} de la macro {} espera un identificador",
                                        param.name, macr.name
                                    ),
                                    span,
                                );
                                self.expand_expr(
                                    &arg.value,
                                    substitutions,
                                    renames,
                                    expansion_stack,
                                    sanitize_locals,
                                )
                            }
                        },
                        _ => self.expand_expr(
                            &arg.value,
                            substitutions,
                            renames,
                            expansion_stack,
                            sanitize_locals,
                        ),
                    };

                    call_substitutions.insert(param.name.clone(), replacement);
                }
            }
        }

        if arg_iter.next().is_some() {
            self.diagnostics.error(
                format!("La macro {} recibe demasiados argumentos", macr.name),
                span,
            );
        }

        if call.action.is_some()
            && !macr
                .params
                .iter()
                .any(|param| matches!(param.kind, MacroParamKind::Block))
        {
            self.diagnostics.error(
                format!("La macro {} no acepta bloque trailing", macr.name),
                span,
            );
        }

        expansion_stack.push(call.name.clone());
        let expanded = self.expand_expr(
            &macr.body,
            &call_substitutions,
            renames,
            expansion_stack,
            true,
        );
        expansion_stack.pop();
        expanded
    }

    fn expand_pattern(
        &mut self,
        pattern: &Pattern,
        sanitize_locals: bool,
    ) -> (Pattern, HashMap<String, String>) {
        match pattern {
            Pattern::Identifier {
                name,
                type_restriction,
            } => {
                if sanitize_locals {
                    let fresh_name = self.fresh_name(name);
                    let mut bindings = HashMap::new();
                    bindings.insert(name.clone(), fresh_name.clone());
                    (
                        Pattern::Identifier {
                            name: fresh_name,
                            type_restriction: type_restriction.clone(),
                        },
                        bindings,
                    )
                } else {
                    (
                        Pattern::Identifier {
                            name: name.clone(),
                            type_restriction: type_restriction.clone(),
                        },
                        HashMap::new(),
                    )
                }
            }
            Pattern::Binary {
                left,
                operator,
                right,
            } => {
                let (left_pattern, left_bindings) = self.expand_pattern(left, sanitize_locals);
                let (right_pattern, right_bindings) = self.expand_pattern(right, sanitize_locals);
                let mut bindings = left_bindings;
                bindings.extend(right_bindings);
                (
                    Pattern::Binary {
                        left: Box::new(left_pattern),
                        operator: operator.clone(),
                        right: Box::new(right_pattern),
                    },
                    bindings,
                )
            }
            Pattern::Unary { operator, operand } => {
                let (operand_pattern, bindings) = self.expand_pattern(operand, sanitize_locals);
                (
                    Pattern::Unary {
                        operator: operator.clone(),
                        operand: Box::new(operand_pattern),
                    },
                    bindings,
                )
            }
            Pattern::Literal(value) => (Pattern::Literal(value.clone()), HashMap::new()),
            Pattern::Default => (Pattern::Default, HashMap::new()),
        }
    }

    fn lookup_rename(&self, renames: &[HashMap<String, String>], name: &str) -> Option<String> {
        for scope in renames.iter().rev() {
            if let Some(renamed) = scope.get(name) {
                return Some(renamed.clone());
            }
        }
        None
    }
}
