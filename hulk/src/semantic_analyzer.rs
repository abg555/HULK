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

#[derive(Clone, Default)]
struct SymbolRequirements {
    concrete: Option<SemanticType>,
    methods: HashMap<String, usize>,
    requires_function: Option<usize>,
    requires_vector: bool,
}

#[derive(Debug)]
pub struct SemanticAnalysis {
    pub inferred_types: HashMap<NodeId, SemanticType>,
}

pub struct SemanticAnalyzer {
    symbols: SymbolTable,
    diagnostics: DiagnosticCollector,
    inferred_types: HashMap<NodeId, SemanticType>,
    assigned_scopes: Vec<HashMap<String, bool>>,
    readonly_scopes: Vec<HashMap<String, String>>,
    current_return_type: Option<SemanticType>,
    current_type_context: Option<String>,
    current_method_context: Option<String>,
    type_parents: HashMap<String, ParentLink>,
    protocol_parents: HashMap<String, ParentLink>,
    type_shapes: HashMap<String, TypeShape>,
    protocol_shapes: HashMap<String, ProtocolShape>,
    inferred_function_params: HashMap<String, HashMap<String, SemanticType>>,
    inferred_function_returns: HashMap<String, SemanticType>,
    inferred_type_params: HashMap<String, HashMap<String, SemanticType>>,
    inferred_method_params: HashMap<String, HashMap<String, HashMap<String, SemanticType>>>,
    inferred_method_returns: HashMap<String, HashMap<String, SemanticType>>,
    synthetic_protocol_counter: usize,
}

impl SemanticAnalyzer {
    const LOOP_FIXPOINT_MAX_ITERS: usize = 3;

    pub fn new() -> Self {
        let mut analyzer = Self {
            symbols: SymbolTable::new(),
            diagnostics: DiagnosticCollector::new(),
            inferred_types: HashMap::new(),
            assigned_scopes: vec![HashMap::new()],
            readonly_scopes: vec![HashMap::new()],
            current_return_type: None,
            current_type_context: None,
            current_method_context: None,
            type_parents: HashMap::new(),
            protocol_parents: HashMap::new(),
            type_shapes: HashMap::new(),
            protocol_shapes: HashMap::new(),
            inferred_function_params: HashMap::new(),
            inferred_function_returns: HashMap::new(),
            inferred_type_params: HashMap::new(),
            inferred_method_params: HashMap::new(),
            inferred_method_returns: HashMap::new(),
            synthetic_protocol_counter: 0,
        };
        analyzer.install_prelude();
        analyzer
    }

    pub fn analyze(mut self, program: &Program) -> Result<SemanticAnalysis, Vec<Diagnostic>> {
        self.collect_top_level(program);
        self.validate_hierarchies();
        self.infer_program_annotations(program);

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

    fn infer_program_annotations(&mut self, program: &Program) {
        self.inferred_function_params.clear();
        self.inferred_function_returns.clear();
        self.inferred_type_params.clear();
        self.inferred_method_params.clear();
        self.inferred_method_returns.clear();

        for item in &program.items {
            match item {
                Item::Function(func) => {
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
                        func
                            .return_type
                            .as_ref()
                            .map(SemanticType::from_type_ref)
                            .unwrap_or(SemanticType::Unknown)
                    } else {
                        self.infer_return_type(&func.body, func.body.span)
                    };
                    self.inferred_function_params.insert(func.name.clone(), inferred);
                    self.inferred_function_returns
                        .insert(func.name.clone(), resolved_return.clone());
                    let _ = self.symbols.update_type(
                        &func.name,
                        SemanticType::Function(resolved_params, Box::new(resolved_return)),
                    );
                }
                Item::Type(typ) => {
                    let inferred_type_params = self.infer_type_decl_params(typ);
                    self.inferred_type_params
                        .insert(typ.name.clone(), inferred_type_params.clone());

                    let mut method_map: HashMap<String, HashMap<String, SemanticType>> = HashMap::new();
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
                            self.infer_return_type(&method.body, method.body.span)
                        };
                        method_return_map.insert(method.name.clone(), ret);
                    }
                    self.inferred_method_params
                        .insert(typ.name.clone(), method_map.clone());
                    self.inferred_method_returns
                        .insert(typ.name.clone(), method_return_map.clone());

                    // Compute all method signatures before updating the shape
                    let mut method_sigs = Vec::new();
                    for method in &typ.methods {
                        let inferred = method_map.get(&method.name).cloned().unwrap_or_default();
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
                        let ret = method_return_map
                            .get(&method.name)
                            .cloned()
                            .unwrap_or(SemanticType::Unknown);
                        method_sigs.push((method.name.clone(), SemanticType::Function(params, Box::new(ret))));
                    }

                    if let Some(shape) = self.type_shapes.get_mut(&typ.name) {
                        shape.ctor_params = typ
                            .param
                            .iter()
                            .map(|param| {
                                param
                                    .types
                                    .as_ref()
                                    .map(SemanticType::from_type_ref)
                                    .unwrap_or_else(|| {
                                        inferred_type_params
                                            .get(&param.name)
                                            .cloned()
                                            .unwrap_or(SemanticType::Unknown)
                                    })
                            })
                            .collect();

                        for (method_name, method_type) in method_sigs {
                            shape.methods.insert(method_name, method_type);
                        }
                    }
                }
                _ => {}
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
                param
                    .types
                    .as_ref()
                    .map(|type_ref| SemanticType::from_type_ref(type_ref))
                    .unwrap_or_else(|| {
                        inferred_param_types
                            .get(&param.name)
                            .cloned()
                            .unwrap_or(SemanticType::Unknown)
                    })
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
            let typ = param
                .types
                .as_ref()
                .map(|type_ref| self.resolve_type_ref(Some(type_ref), func.body.span))
                .unwrap_or_else(|| {
                    inferred_param_types
                        .get(&param.name)
                        .cloned()
                        .unwrap_or(SemanticType::Unknown)
                });
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

        // Validate parent constructor arguments if parent exists
        if let Some(parent) = &typ.parent {
            if let SemanticType::Custom(parent_name) = self.resolve_type_ref(Some(parent), self.type_decl_span(typ)) {
                if let Some(parent_shape) = self.type_shapes.get(&parent_name).cloned() {
                    self.validate_parent_constructor_args(typ, &parent_name, &parent_shape);
                }
            }
        }

        let inferred_type_params = self
            .inferred_type_params
            .get(&typ.name)
            .cloned()
            .unwrap_or_else(|| self.infer_type_decl_params(typ));
        if let Some(shape) = self.type_shapes.get_mut(&typ.name) {
            shape.ctor_params = typ
                .param
                .iter()
                .map(|param| {
                    param
                        .types
                        .as_ref()
                        .map(SemanticType::from_type_ref)
                        .unwrap_or_else(|| {
                            inferred_type_params
                                .get(&param.name)
                                .cloned()
                                .unwrap_or(SemanticType::Unknown)
                        })
                })
                .collect();
        }

        self.enter_scope();
        let prev_type_context = self.current_type_context.clone();
        self.current_type_context = Some(typ.name.clone());

        for param in &typ.param {
            let param_ty = param
                .types
                .as_ref()
                .map(SemanticType::from_type_ref)
                .unwrap_or_else(|| {
                    inferred_type_params
                        .get(&param.name)
                        .cloned()
                        .unwrap_or(SemanticType::Unknown)
                });
            self.define_local_with_state(
                &param.name,
                SymbolKind::Variable,
                param_ty,
                self.type_decl_span(typ),
                true,
            );
            self.mark_local_readonly(&param.name, "argumento de tipo es de solo lectura");
        }

        let mut field_names = HashSet::new();
        let mut initialized_fields: HashSet<String> = HashSet::new();
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

            // Check that the field initializer does not access later fields via `self.<field>`
            let mut accessed: Vec<String> = Vec::new();
            self.collect_self_member_accesses(&field.initializer, &mut accessed);
            for acc in accessed {
                if acc == field.name {
                    self.diagnostics.warning(
                        format!(
                            "El inicializador del campo {} en {} referencia a sí mismo",
                            field.name, typ.name
                        ),
                        field.initializer.span,
                    );
                    continue;
                }

                if initialized_fields.contains(&acc) {
                    continue; // referencing previously-initialized sibling field is OK
                }

                // referencing inherited field is OK
                if self.lookup_member_type_in_parent_chain(&typ.name, &acc).is_some() {
                    continue;
                }

                // otherwise it's accessing a field declared later -> emit warning with hint to declaration if available
                let mut diag = Diagnostic::warning(
                    format!(
                        "El inicializador del campo {} en {} accede al campo {} declarado después (inicialización parcial)",
                        field.name, typ.name, acc
                    ),
                    field.initializer.span,
                );
                // try to find the declaration span of the referenced field in this type
                if let Some(decl) = typ.fields.iter().find(|f| f.name == acc) {
                    let hint = format!("Declarado en span: {}..{}", decl.initializer.span.start, decl.initializer.span.end);
                    diag = diag.with_hint(hint);
                }
                self.diagnostics.push(diag);
            }

            if !self.guarantees_value(&field.initializer) {
                self.diagnostics.error(
                    format!(
                        "El inicializador del campo {} en {} no garantiza valor",
                        field.name, typ.name
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
            self.define_local_with_state(
                &field.name,
                SymbolKind::Variable,
                declared,
                field.initializer.span,
                self.guarantees_value(&field.initializer),
            );
            initialized_fields.insert(field.name.clone());
        }

        for method in &typ.methods {
            if !method_names.insert(method.name.clone()) {
                self.diagnostics.error(
                    format!("Metodo duplicado en {}: {}", typ.name, method.name),
                    method.body.span,
                );
            }

            self.validate_method_override(typ, method);
            let prev_method_context = self.current_method_context.clone();
            self.current_method_context = Some(method.name.clone());
            self.check_function_decl(method);
            self.current_method_context = prev_method_context;
        }

        self.current_type_context = prev_type_context;
        self.exit_scope();
    }

    fn infer_type_decl_params(&mut self, typ: &TypeDecl) -> HashMap<String, SemanticType> {
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
            let mut shadow_stack = vec![method.params.iter().map(|param| param.name.clone()).collect()];
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
            inferred.insert(name.clone(), self.synthesize_inferred_type(&name, req, self.type_decl_span(typ)));
        }

        inferred
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
        self.enter_scope();
        for param in &macr.params {
            let typ = self.resolve_type_ref(param.type_info.as_ref(), macr.body.span);
            self.define_local(&param.name, SymbolKind::Variable, typ, macr.body.span);
        }
        self.check_expr(&macr.body);
        self.exit_scope();
    }

    fn infer_param_types(&mut self, func: &FunctionDecl) -> HashMap<String, SemanticType> {
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

    fn collect_inference_requirements(
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
                               diagnostics: &mut DiagnosticCollector,
                               span: Span| {
            let entry = requirements.entry(name.to_string()).or_default();
            if let Some(existing) = &entry.concrete {
                if existing != &ty && !existing.is_assignable_from(&ty) && !ty.is_assignable_from(existing) {
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
                self.collect_inference_requirements(&bin.left, inferable, shadow_stack, requirements);
                self.collect_inference_requirements(&bin.right, inferable, shadow_stack, requirements);

                let left_name = match &bin.left.kind {
                    KindExpr::Variable(var) if inferable.contains(&var.name) && !is_shadowed(&var.name, shadow_stack) => Some(var.name.clone()),
                    _ => None,
                };
                let right_name = match &bin.right.kind {
                    KindExpr::Variable(var) if inferable.contains(&var.name) && !is_shadowed(&var.name, shadow_stack) => Some(var.name.clone()),
                    _ => None,
                };

                match bin.operator {
                    BinaryOperator::Add
                    | BinaryOperator::Sub
                    | BinaryOperator::Mul
                    | BinaryOperator::Div
                    | BinaryOperator::Pow
                    | BinaryOperator::Mod => {
                        if let Some(name) = left_name {
                            record_concrete(requirements, &name, SemanticType::Number, &mut self.diagnostics, expr.span);
                        }
                        if let Some(name) = right_name {
                            record_concrete(requirements, &name, SemanticType::Number, &mut self.diagnostics, expr.span);
                        }
                    }
                    BinaryOperator::And | BinaryOperator::Or => {
                        if let Some(name) = left_name {
                            record_concrete(requirements, &name, SemanticType::Boolean, &mut self.diagnostics, expr.span);
                        }
                        if let Some(name) = right_name {
                            record_concrete(requirements, &name, SemanticType::Boolean, &mut self.diagnostics, expr.span);
                        }
                    }
                    BinaryOperator::Concat | BinaryOperator::FullConcat => {
                        if let Some(name) = left_name {
                            record_concrete(requirements, &name, SemanticType::String, &mut self.diagnostics, expr.span);
                        }
                        if let Some(name) = right_name {
                            record_concrete(requirements, &name, SemanticType::String, &mut self.diagnostics, expr.span);
                        }
                    }
                    BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::Greater
                    | BinaryOperator::LessEqual
                    | BinaryOperator::GreaterEqual => {}
                }
            }
            KindExpr::Unary(unary) => {
                self.collect_inference_requirements(&unary.right, inferable, shadow_stack, requirements);
                if let KindExpr::Variable(var) = &unary.right.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    let ty = match unary.operator {
                        UnaryOperator::Negate => SemanticType::Number,
                        UnaryOperator::Not => SemanticType::Boolean,
                    };
                    record_concrete(requirements, &var.name, ty, &mut self.diagnostics, expr.span);
                }
            }
            KindExpr::Call(call) => {
                self.collect_inference_requirements(&call.callee, inferable, shadow_stack, requirements);
                for arg in &call.arguments {
                    self.collect_inference_requirements(arg, inferable, shadow_stack, requirements);
                }

                if let KindExpr::MemberAccess(member) = &call.callee.kind
                    && let KindExpr::Variable(var) = &member.object.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    requirements.entry(var.name.clone()).or_default().methods.insert(member.field.clone(), call.arguments.len());
                } else if let KindExpr::Variable(var) = &call.callee.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    requirements.entry(var.name.clone()).or_default().requires_function = Some(call.arguments.len());
                }
            }
            KindExpr::BaseCall(base_call) => {
                for arg in &base_call.arguments {
                    self.collect_inference_requirements(arg, inferable, shadow_stack, requirements);
                }
            }
            KindExpr::MacroCall(macr) => {
                for arg in &macr.arguments {
                    self.collect_inference_requirements(&arg.value, inferable, shadow_stack, requirements);
                }
                if let Some(action) = &macr.action {
                    self.collect_inference_requirements(action, inferable, shadow_stack, requirements);
                }
            }
            KindExpr::Let(let_expr) => {
                for binding in &let_expr.bindings {
                    self.collect_inference_requirements(&binding.initializer, inferable, shadow_stack, requirements);
                    let mut scope = HashSet::new();
                    scope.insert(binding.name.clone());
                    shadow_stack.push(scope);
                }
                self.collect_inference_requirements(&let_expr.body, inferable, shadow_stack, requirements);
                for _ in &let_expr.bindings {
                    shadow_stack.pop();
                }
            }
            KindExpr::Block(block) => {
                for expr in &block.expressions {
                    self.collect_inference_requirements(expr, inferable, shadow_stack, requirements);
                }
            }
            KindExpr::If(if_expr) => {
                self.collect_inference_requirements(&if_expr.condition, inferable, shadow_stack, requirements);
                self.collect_inference_requirements(&if_expr.then_branch, inferable, shadow_stack, requirements);
                for (cond, body) in &if_expr.elif_branches {
                    self.collect_inference_requirements(cond, inferable, shadow_stack, requirements);
                    self.collect_inference_requirements(body, inferable, shadow_stack, requirements);
                }
                self.collect_inference_requirements(&if_expr.else_branch, inferable, shadow_stack, requirements);
            }
            KindExpr::While(while_expr) => {
                self.collect_inference_requirements(&while_expr.condition, inferable, shadow_stack, requirements);
                self.collect_inference_requirements(&while_expr.body, inferable, shadow_stack, requirements);
            }
            KindExpr::For(for_expr) => {
                self.collect_inference_requirements(&for_expr.iterable, inferable, shadow_stack, requirements);
                let mut scope = HashSet::new();
                scope.insert(for_expr.variable.clone());
                shadow_stack.push(scope);
                self.collect_inference_requirements(&for_expr.body, inferable, shadow_stack, requirements);
                shadow_stack.pop();
            }
            KindExpr::Assign(assign) => {
                self.collect_inference_requirements(&assign.target, inferable, shadow_stack, requirements);
                self.collect_inference_requirements(&assign.value, inferable, shadow_stack, requirements);
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
                    record_concrete(requirements, &var.name, value_ty, &mut self.diagnostics, expr.span);
                }
            }
            KindExpr::MemberAccess(member) => {
                self.collect_inference_requirements(&member.object, inferable, shadow_stack, requirements);
            }
            KindExpr::Index(index) => {
                self.collect_inference_requirements(&index.object, inferable, shadow_stack, requirements);
                self.collect_inference_requirements(&index.index, inferable, shadow_stack, requirements);
                if let KindExpr::Variable(var) = &index.object.kind
                    && inferable.contains(&var.name)
                    && !is_shadowed(&var.name, shadow_stack)
                {
                    requirements.entry(var.name.clone()).or_default().requires_vector = true;
                }
            }
            KindExpr::Array(array) => {
                for element in &array.elements {
                    self.collect_inference_requirements(element, inferable, shadow_stack, requirements);
                }
            }
            KindExpr::ArrayComprehension(comp) => {
                self.collect_inference_requirements(&comp.iterable, inferable, shadow_stack, requirements);
                let mut scope = HashSet::new();
                scope.insert(comp.variable.clone());
                shadow_stack.push(scope);
                self.collect_inference_requirements(&comp.element, inferable, shadow_stack, requirements);
                shadow_stack.pop();
            }
            KindExpr::Lambda(lambda) => {
                for param in &lambda.params {
                    let mut scope = HashSet::new();
                    scope.insert(param.name.clone());
                    shadow_stack.push(scope);
                }
                self.collect_inference_requirements(&lambda.body, inferable, shadow_stack, requirements);
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
                self.collect_inference_requirements(&is_expr.expression, inferable, shadow_stack, requirements);
            }
            KindExpr::As(as_expr) => {
                self.collect_inference_requirements(&as_expr.expression, inferable, shadow_stack, requirements);
            }
            KindExpr::Match(match_expr) => {
                self.collect_inference_requirements(&match_expr.expression, inferable, shadow_stack, requirements);
                for case in &match_expr.cases {
                    self.collect_inference_requirements(&case.body, inferable, shadow_stack, requirements);
                }
            }
        }
    }

    fn synthesize_inferred_type(
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
                ProtocolShape {
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

    fn next_synthetic_protocol_name(&mut self) -> String {
        self.synthetic_protocol_counter += 1;
        format!("_P{}", self.synthetic_protocol_counter)
    }

    fn check_expr(&mut self, expr: &Expr) -> SemanticType {
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
                } else
                if let Some(symbol) = self.symbols.lookup(&var.name) {
                    let symbol_kind = symbol.kind;
                    let symbol_type = symbol.typ.clone();
                    if symbol_kind == SymbolKind::Variable && !self.is_definitely_assigned(&var.name) {
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
            KindExpr::BaseCall(call) => self.check_base_call(call, expr.span),
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
                self.enter_scope();
                let body_inferable = let_expr
                    .bindings
                    .iter()
                    .filter(|binding| binding.types.is_none())
                    .map(|binding| binding.name.clone())
                    .collect::<HashSet<_>>();

                for binding in &let_expr.bindings {
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

                    let init_ty = self.check_expr(&binding.initializer);
                    let declared =
                        self.resolve_type_ref(binding.types.as_ref(), binding.initializer.span);
                    let final_ty = if binding.types.is_none() && matches!(init_ty, SemanticType::Unknown) {
                        let mut requirements: HashMap<String, SymbolRequirements> = HashMap::new();
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
                    self.define_local_with_state(
                        &binding.name,
                        SymbolKind::Variable,
                        final_ty,
                        binding.initializer.span,
                        initializer_guaranteed,
                    );
                }
                let body_ty = self.check_expr(&let_expr.body);
                self.exit_scope();
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
            KindExpr::If(if_expr) => {
                self.report_unreachable_if_branches(if_expr, expr.span);

                let state_before_if = self.assigned_scopes.clone();
                self.assigned_scopes = state_before_if.clone();
                let cond_ty = self.check_expr(&if_expr.condition);
                self.expect_type(expr.span, &cond_ty, &SemanticType::Boolean, "condicion de if");
                self.assigned_scopes = state_before_if.clone();

                let then_ty = if let Some((name, narrowed_type)) =
                    self.extract_is_narrowing(&if_expr.condition, expr.span)
                {
                    self.enter_scope();
                    self.define_local(&name, SymbolKind::Variable, narrowed_type, expr.span);
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
                    self.expect_type(expr.span, &elif_cond_ty, &SemanticType::Boolean, "condicion de elif");
                    self.assigned_scopes = state_before_if.clone();
                    elif_types.push(self.check_expr(elif_body));
                    branch_states.push(self.assigned_scopes.clone());
                }

                self.assigned_scopes = state_before_if.clone();
                let else_ty = self.check_expr(&if_expr.else_branch);
                branch_states.push(self.assigned_scopes.clone());
                self.assigned_scopes = self.merge_definite_assignment_states(&state_before_if, &branch_states);

                let mut combined_ty = then_ty;
                for elif_ty in elif_types {
                    if self.is_compatible_type(&combined_ty, &elif_ty) {
                        continue;
                    }
                    if self.is_compatible_type(&elif_ty, &combined_ty) {
                        combined_ty = elif_ty;
                        continue;
                    }
                    combined_ty = SemanticType::Unknown;
                    break;
                }
                if self.is_compatible_type(&combined_ty, &else_ty) {
                    combined_ty
                } else if self.is_compatible_type(&else_ty, &combined_ty) {
                    else_ty
                } else {
                    SemanticType::Unknown
                }
            }
            KindExpr::While(while_expr) => {
                let min_iterations = self.while_min_iterations(&while_expr.condition);
                if matches!(min_iterations, Some(0)) {
                    self.diagnostics.error(
                        "Cuerpo de while inalcanzable: la condicion es siempre false",
                        while_expr.body.span,
                    );
                }
                let cond_ty = self.check_expr(&while_expr.condition);
                self.expect_type(expr.span, &cond_ty, &SemanticType::Boolean, "condicion de while");
                let state_after_condition_eval = self.assigned_scopes.clone();

                let mut loop_state = state_after_condition_eval.clone();
                for _ in 0..Self::LOOP_FIXPOINT_MAX_ITERS {
                    self.assigned_scopes = loop_state.clone();
                    self.check_expr(&while_expr.body);
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
                SemanticType::Unknown
            }
            KindExpr::For(for_expr) => {
                let min_iterations = self.iterable_min_iterations(&for_expr.iterable);
                let iterable_ty = self.check_expr(&for_expr.iterable);
                let state_after_iterable_eval = self.assigned_scopes.clone();

                let element_ty = match iterable_ty {
                    SemanticType::Vector(inner) => *inner,
                    SemanticType::Custom(ref type_name) => {
                        // Check if the type conforms to Iterable protocol
                        if self.type_conforms_to_protocol(type_name, "Iterable") {
                            // For now, assume element type is Unknown for custom iterables
                            // In the future, this could be refined to extract the element type from the protocol
                            SemanticType::Unknown
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
                for _ in 0..Self::LOOP_FIXPOINT_MAX_ITERS {
                    self.assigned_scopes = loop_state.clone();
                    self.enter_scope();
                    self.define_local(&for_expr.variable, SymbolKind::Variable, element_ty.clone(), expr.span);
                    self.mark_local_readonly(&for_expr.variable, "iterador de for");
                    self.check_expr(&for_expr.body);
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
                    None => {
                        self.intersect_definite_assignment_states(
                            &state_after_iterable_eval,
                            &loop_state,
                        )
                    }
                };
                SemanticType::Unknown
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
                    self.define_local(&param.name, SymbolKind::Variable, param_ty.clone(), expr.span);
                    params.push(param_ty);
                }
                let body_ty = self.check_expr(&lambda.body);
                self.exit_scope();
                let ret = lambda
                    .return_type
                    .as_ref()
                    .map(SemanticType::from_type_ref)
                    .unwrap_or(body_ty);

                if let Some(expected_ret) = lambda.return_type.as_ref().map(SemanticType::from_type_ref)
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
                    let current_pattern_ty = self.pattern_coverage_type(&case.pattern, expr.span);
                    let shadowed_by_previous_type = current_pattern_ty.as_ref().is_some_and(|current| {
                        seen_type_coverages
                            .iter()
                            .any(|previous| self.is_compatible_type(previous, current))
                    });
                    let known_literal_match = known_scrutinee_literal
                        .and_then(|literal| self.pattern_matches_known_literal(&case.pattern, literal, expr.span));
                    let impossible_for_known_literal = matches!(known_literal_match, Some(false));

                    if saw_default_case
                        || saw_catch_all_pattern
                        || saw_forced_match_case
                        || shadowed_by_previous_type
                        || impossible_for_known_literal
                    {
                        self.diagnostics.error(
                            "Caso inalcanzable: hay un case previo que ya cubre este patron",
                            expr.span,
                        );
                    }

                    self.assigned_scopes = state_before_cases.clone();
                    self.enter_scope();
                    self.check_pattern(&case.pattern, &scrutinee_ty, scrutinee_name, expr.span);
                    let case_ty = self.check_expr(&case.body);
                    self.exit_scope();
                    case_states.push(self.assigned_scopes.clone());

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
                                    expr.span,
                                );
                            }
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
                    } else if !self.is_compatible_type(&merged, &case_ty)
                        && !self.is_compatible_type(&case_ty, &merged)
                    {
                        merged = SemanticType::Unknown;
                    }
                }

                if matches!(scrutinee_ty, SemanticType::Boolean)
                    && !saw_default_case
                    && !saw_forced_match_case
                    && !(saw_true_case && saw_false_case)
                {
                    self.diagnostics.error(
                        "Match sobre Boolean no exhaustivo: faltan casos true/false o default",
                        expr.span,
                    );
                }

                if !matches!(scrutinee_ty, SemanticType::Boolean | SemanticType::Unknown)
                    && !saw_default_case
                    && !saw_forced_match_case
                {
                    self.diagnostics.error(
                        format!("Match no exhaustivo para {}: falta default", scrutinee_ty),
                        expr.span,
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
                self.mark_local_readonly(name, "binding de patron de match");

                if let Some(scrutinee_name) = scrutinee_name
                    && scrutinee_name != name
                {
                    self.define_local(
                        scrutinee_name,
                        SymbolKind::Variable,
                        bound_type,
                        span,
                    );
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

    fn guarantees_value(&self, expr: &Expr) -> bool {
        match &expr.kind {
            KindExpr::While(_) | KindExpr::For(_) => false,
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

    fn literal_type(&self, literal: &LiteralValue) -> SemanticType {
        match literal {
            LiteralValue::Number(_) => SemanticType::Number,
            LiteralValue::String(_) => SemanticType::String,
            LiteralValue::Bool(_) => SemanticType::Boolean,
        }
    }

    fn literal_pattern_key(literal: &LiteralValue) -> String {
        match literal {
            LiteralValue::Number(n) => n.to_string(),
            LiteralValue::String(s) => format!("\"{}\"", s),
            LiteralValue::Bool(b) => b.to_string(),
        }
    }

    fn literal_values_equal(&self, left: &LiteralValue, right: &LiteralValue) -> bool {
        match (left, right) {
            (LiteralValue::Number(a), LiteralValue::Number(b)) => a.to_bits() == b.to_bits(),
            (LiteralValue::String(a), LiteralValue::String(b)) => a == b,
            (LiteralValue::Bool(a), LiteralValue::Bool(b)) => a == b,
            _ => false,
        }
    }

    fn pattern_matches_known_literal(
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

    fn pattern_coverage_type(&mut self, pattern: &Pattern, span: Span) -> Option<SemanticType> {
        match pattern {
            Pattern::Identifier {
                type_restriction: Some(type_ref),
                ..
            } => Some(self.resolve_type_ref(Some(type_ref), span)),
            _ => None,
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

    fn check_assignment_target(&mut self, target: &Expr) -> SemanticType {
        if let KindExpr::Variable(var) = &target.kind {
            if let Some(symbol) = self.symbols.lookup(&var.name) {
                return symbol.typ.clone();
            }

            self.diagnostics
                .error(format!("Identificador no definido: {}", var.name), target.span);
            return SemanticType::Unknown;
        }

        self.check_expr(target)
    }

    fn check_base_call(&mut self, call: &BaseCallExpr, span: Span) -> SemanticType {
        let Some(type_name) = self.current_type_context.clone() else {
            self.diagnostics
                .error("'base(...)' solo es valido dentro de metodos de tipo", span);
            for arg in &call.arguments {
                self.check_expr(arg);
            }
            return SemanticType::Unknown;
        };

        let Some(method_name) = self.current_method_context.clone() else {
            self.diagnostics
                .error("'base(...)' solo es valido dentro del cuerpo de un metodo", span);
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

        let Some(parent_signature) = self.lookup_member_type_in_parent_chain(&type_name, &method_name) else {
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
                    .or_else(|| {
                        self.inferred_method_returns
                            .get(&typ.name)
                            .and_then(|methods| methods.get(&method.name))
                            .cloned()
                    })
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

    fn validate_parent_constructor_args(&mut self, typ: &TypeDecl, parent_name: &str, parent_shape: &TypeShape) {
        if parent_shape.ctor_params.len() != typ.parent_arg.len() {
            self.diagnostics.error(
                format!(
                    "Constructor de {} espera {} argumentos en la herencia de {}, pero se proporcionan {}",
                    parent_name,
                    parent_shape.ctor_params.len(),
                    typ.name,
                    typ.parent_arg.len()
                ),
                if let Some(expr) = typ.parent_arg.first() {
                    expr.span
                } else {
                    self.type_decl_span(typ)
                },
            );
        }

        for (idx, (expected, arg_expr)) in parent_shape.ctor_params.iter().zip(typ.parent_arg.iter()).enumerate() {
            let actual = self.check_expr(arg_expr);
            if !self.is_compatible_type(expected, &actual) {
                self.diagnostics.error(
                    format!(
                        "Argumento {} del constructor padre {} espera {}, pero recibe {}",
                        idx + 1,
                        parent_name,
                        expected,
                        actual
                    ),
                    arg_expr.span,
                );
                self.diagnostics.error(
                    format!(
                        "Constructor padre {} incompatible: argumento {} incompatible",
                        parent_name,
                        idx + 1
                    ),
                    arg_expr.span,
                );
            }
        }
    }

    fn collect_self_member_accesses(&self, expr: &Expr, out: &mut Vec<String>) {
        use KindExpr::*;
        match &expr.kind {
            MemberAccess(member) => {
                if let KindExpr::Variable(var) = &member.object.kind {
                    if var.name == "self" {
                        out.push(member.field.clone());
                        // still descend the object in case of chained accesses
                        self.collect_self_member_accesses(&member.object, out);
                        return;
                    }
                }
                // descend into object and no further for field name
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
            Call(_) | BaseCall(_) | Literal(_) | Variable(_) => {}
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

        // Predefined Iterable protocol
        self.define_builtin_protocol("Iterable");
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

    fn define_builtin_protocol(&mut self, name: &str) {
        let _ = self.symbols.define(Symbol {
            name: name.to_string(),
            kind: SymbolKind::Protocol,
            typ: SemanticType::Unknown,
        });
        self.protocol_shapes.insert(
            name.to_string(),
            ProtocolShape {
                parent: None,
                methods: HashMap::new(),
            },
        );
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

    fn enter_scope(&mut self) {
        self.symbols.enter_scope();
        self.assigned_scopes.push(HashMap::new());
        self.readonly_scopes.push(HashMap::new());
    }

    fn exit_scope(&mut self) {
        self.symbols.exit_scope();
        if self.assigned_scopes.len() > 1 {
            self.assigned_scopes.pop();
        }
        if self.readonly_scopes.len() > 1 {
            self.readonly_scopes.pop();
        }
    }

    fn is_definitely_assigned(&self, name: &str) -> bool {
        self.assigned_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
            .unwrap_or(true)
    }

    fn collect_return_expressions<'a>(&self, expr: &'a Expr) -> Vec<&'a Expr> {
        let mut returns = Vec::new();
        self.collect_returns_helper(expr, &mut returns);
        returns
    }

    fn collect_returns_helper<'a>(&self, expr: &'a Expr, returns: &mut Vec<&'a Expr>) {
        match &expr.kind {
            KindExpr::Block(block) => {
                if block.expressions.is_empty() {
                    returns.push(expr);
                } else {
                    // The return type is determined by the last expression in the block
                    self.collect_returns_helper(&block.expressions[block.expressions.len() - 1], returns);
                }
            }
            KindExpr::If(if_expr) => {
                // Collect returns from all branches
                self.collect_returns_helper(&if_expr.then_branch, returns);
                for (_, body) in &if_expr.elif_branches {
                    self.collect_returns_helper(body, returns);
                }
                self.collect_returns_helper(&if_expr.else_branch, returns);
            }
            KindExpr::Let(let_expr) => {
                // The return type is determined by the body of the let expression
                self.collect_returns_helper(&let_expr.body, returns);
            }
            KindExpr::Match(match_expr) => {
                // Collect returns from all match cases
                for case in &match_expr.cases {
                    self.collect_returns_helper(&case.body, returns);
                }
            }
            _ => {
                // Base case: this is the return expression
                returns.push(expr);
            }
        }
    }

    fn infer_return_type(&self, body: &Expr, _span: Span) -> SemanticType {
        let return_exprs = self.collect_return_expressions(body);

        if return_exprs.is_empty() {
            return SemanticType::Unknown;
        }

        // Collect types from all return expressions
        let mut return_types = Vec::new();
        for ret_expr in return_exprs {
            match &ret_expr.kind {
                KindExpr::Literal(lit) => match lit.value {
                    LiteralValue::Number(_) => return_types.push(SemanticType::Number),
                    LiteralValue::String(_) => return_types.push(SemanticType::String),
                    LiteralValue::Bool(_) => return_types.push(SemanticType::Boolean),
                },
                KindExpr::Block(block) if block.expressions.is_empty() => {
                    return_types.push(SemanticType::Boolean); // Empty block evaluates to false
                }
                KindExpr::Array(_) => return_types.push(SemanticType::Vector(Box::new(SemanticType::Unknown))),
                _ => return_types.push(SemanticType::Unknown),
            }
        }

        // Unify return types
        if return_types.is_empty() {
            return SemanticType::Unknown;
        }

        let first = return_types[0].clone();
        let all_same = return_types.iter().all(|t| {
            match (t, &first) {
                (SemanticType::Number, SemanticType::Number) => true,
                (SemanticType::String, SemanticType::String) => true,
                (SemanticType::Boolean, SemanticType::Boolean) => true,
                (SemanticType::Vector(_), SemanticType::Vector(_)) => true,
                (SemanticType::Unknown, _) => true,
                (_, SemanticType::Unknown) => true,
                _ => false,
            }
        });

        if all_same {
            first
        } else {
            SemanticType::Unknown
        }
    }

    fn mark_assigned(&mut self, name: &str) {
        for scope in self.assigned_scopes.iter_mut().rev() {
            if let Some(state) = scope.get_mut(name) {
                *state = true;
                return;
            }
        }
    }

    fn readonly_reason(&self, name: &str) -> Option<&str> {
        self.readonly_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).map(String::as_str))
    }

    fn mark_local_readonly(&mut self, name: &str, reason: impl Into<String>) {
        if let Some(scope) = self.readonly_scopes.last_mut() {
            scope.insert(name.to_string(), reason.into());
        }
    }

    fn merge_definite_assignment_states(
        &self,
        before: &[HashMap<String, bool>],
        branch_states: &[Vec<HashMap<String, bool>>],
    ) -> Vec<HashMap<String, bool>> {
        let mut merged = before.to_vec();

        for (scope_idx, merged_scope) in merged.iter_mut().enumerate() {
            let keys = merged_scope.keys().cloned().collect::<Vec<_>>();
            for name in keys {
                let before_val = before
                    .get(scope_idx)
                    .and_then(|scope| scope.get(&name))
                    .copied()
                    .unwrap_or(false);
                let assigned_in_all_branches = branch_states.iter().all(|state| {
                    state
                        .get(scope_idx)
                        .and_then(|scope| scope.get(&name))
                        .copied()
                        .unwrap_or(false)
                });
                merged_scope.insert(name, before_val || assigned_in_all_branches);
            }
        }

        merged
    }

    fn intersect_definite_assignment_states(
        &self,
        left: &[HashMap<String, bool>],
        right: &[HashMap<String, bool>],
    ) -> Vec<HashMap<String, bool>> {
        let mut merged = left.to_vec();

        for (scope_idx, merged_scope) in merged.iter_mut().enumerate() {
            let keys = merged_scope.keys().cloned().collect::<Vec<_>>();
            for name in keys {
                let left_val = left
                    .get(scope_idx)
                    .and_then(|scope| scope.get(&name))
                    .copied()
                    .unwrap_or(false);
                let right_val = right
                    .get(scope_idx)
                    .and_then(|scope| scope.get(&name))
                    .copied()
                    .unwrap_or(false);
                merged_scope.insert(name, left_val && right_val);
            }
        }

        merged
    }

    fn iterable_min_iterations(&self, iterable: &Expr) -> Option<usize> {
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

    fn while_min_iterations(&self, condition: &Expr) -> Option<usize> {
        self.eval_const_bool(condition)
            .map(|cond| if cond { 1 } else { 0 })
    }

    fn eval_const_number(&self, expr: &Expr) -> Option<f64> {
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

    fn eval_const_literal(&self, expr: &Expr) -> Option<LiteralValue> {
        let KindExpr::Literal(lit) = &expr.kind else {
            return None;
        };

        match &lit.value {
            LiteralValue::Number(n) => Some(LiteralValue::Number(*n)),
            LiteralValue::String(s) => Some(LiteralValue::String(s.clone())),
            LiteralValue::Bool(b) => Some(LiteralValue::Bool(*b)),
        }
    }

    fn eval_const_bool(&self, expr: &Expr) -> Option<bool> {
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
                BinaryOperator::And => Some(
                    self.eval_const_bool(&bin.left)? && self.eval_const_bool(&bin.right)?,
                ),
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

    fn report_unreachable_if_branches(&mut self, if_expr: &IfExpr, span: Span) {
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

    fn definitely_non_terminating(&self, expr: &Expr) -> bool {
        match &expr.kind {
            KindExpr::While(while_expr) => matches!(self.eval_const_bool(&while_expr.condition), Some(true)),
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
        self.define_local_with_state(name, kind, typ, span, true);
    }

    fn define_local_with_state(
        &mut self,
        name: &str,
        kind: SymbolKind,
        typ: SemanticType,
        span: Span,
        is_assigned: bool,
    ) {
        // warn if this definition shadows a symbol in an outer scope
        if let Some(existing) = self.symbols.lookup(name) {
            self.diagnostics.warning(
                format!("Sombra de simbolo: '{}' oculta un simbolo externo de tipo {:?}", name, existing.kind),
                span,
            );
        }

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
            return;
        }

        if kind == SymbolKind::Variable
            && let Some(current_scope) = self.assigned_scopes.last_mut()
        {
            current_scope.insert(name.to_string(), is_assigned);
        }
    }
}
