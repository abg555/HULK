use std::collections::{HashMap, HashSet};
use std::fs;

use crate::ast::*;
use crate::diagnostics::{Diagnostic, DiagnosticCollector};
use crate::semantic::symbol_table::{Symbol, SymbolKind, SymbolTable};
use crate::semantic::types::SemanticType;

// Internal modules
mod expr;
mod flow;
pub mod functor_desugar;
mod inference;
pub mod macro_expander;
mod scope;
pub mod symbol_table;
mod type_checks;
pub mod types;

/// Enlace entre un tipo o protocolo hijo y su padre declarado.
#[derive(Clone)]
struct ParentLink {
    parent: String,
    span: Span,
}

/// Forma estructural de un tipo declarado: constructor, campos, metodos y herencia.
#[derive(Clone, Debug)]
pub struct TypeShape {
    pub ctor_params: Vec<SemanticType>,
    pub fields: HashMap<String, SemanticType>,
    pub methods: HashMap<String, SemanticType>,
    pub parent: Option<String>,
}

/// Forma estructural de un protocolo declarado.
#[derive(Clone, Debug)]
pub struct ProtocolShape {
    pub methods: HashMap<String, SemanticType>,
    pub parent: Option<String>,
}

/// Restricciones acumuladas durante la inferencia de tipos.
#[derive(Clone, Default)]
struct SymbolRequirements {
    concrete: Option<SemanticType>,
    methods: HashMap<String, usize>,
    requires_function: Option<usize>,
    requires_vector: bool,
}

/// Contexto semantico de solo lectura para fases posteriores como codegen.
#[derive(Debug, Clone)]
pub struct SemanticContext {
    pub inferred_types: HashMap<NodeId, SemanticType>,
    pub type_shapes: HashMap<String, TypeShape>,
    pub protocol_shapes: HashMap<String, ProtocolShape>,
    pub global_symbols: HashMap<String, Symbol>,
    pub imports: Vec<String>,
    pub exports: Vec<String>,
}

/// Alias de compatibilidad para codigo que ya consume SemanticAnalysis.
pub type SemanticAnalysis = SemanticContext;

/// Orquesta las pasadas semanticas, el analisis de flujo y la inferencia de tipos.
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
    imports: Vec<String>,
    exports: Vec<String>,
    /// Cache of analyzed modules by module name.
    module_cache: HashMap<String, SemanticContext>,
    /// Map of module name -> its public symbols snapshot.
    namespaces: HashMap<String, HashMap<String, Symbol>>,
    /// Modules currently being loaded (to detect cycles).
    loading_modules: HashSet<String>,
}

impl SemanticAnalyzer {
    /// Maximo de iteraciones usadas por el punto fijo de validacion en bucles.
    const LOOP_FIXPOINT_MAX_ITERS: usize = 3;

    /// Crea un analizador nuevo y carga el preludio con simbolos integrados.
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
            imports: Vec::new(),
            exports: Vec::new(),
            module_cache: HashMap::new(),
            namespaces: HashMap::new(),
            loading_modules: HashSet::new(),
        };
        analyzer.install_prelude();
        analyzer
    }

    fn with_module_state(
        module_cache: HashMap<String, SemanticContext>,
        namespaces: HashMap<String, HashMap<String, Symbol>>,
        loading_modules: HashSet<String>,
    ) -> Self {
        let mut analyzer = Self::new();
        analyzer.module_cache = module_cache;
        analyzer.namespaces = namespaces;
        analyzer.loading_modules = loading_modules;
        analyzer
    }

    fn build_public_namespace(ctx: &SemanticContext) -> HashMap<String, Symbol> {
        let exported: HashSet<String> = ctx.exports.iter().cloned().collect();
        let has_explicit_exports = !exported.is_empty();

        // Expose only the public API declared by the module. When a module
        // has no export declarations yet, keep the historical behavior and
        // publish all top-level symbols.
        let mut public = HashMap::new();
        for (name, sym) in &ctx.global_symbols {
            if !has_explicit_exports || exported.contains(name) {
                public.insert(name.clone(), sym.clone());
            }
        }
        public
    }

    /// Load a module from disk (module name uses '.' as directory separator) and
    /// register its public symbols in `namespaces`. Reports diagnostics on failure.
    fn load_module(&mut self, module: &str) -> bool {
        if self.namespaces.contains_key(module) {
            return true;
        }
        if let Some(cached) = self.module_cache.get(module).cloned() {
            self.namespaces
                .insert(module.to_string(), Self::build_public_namespace(&cached));
            return true;
        }
        if self.loading_modules.contains(module) {
            self.diagnostics.error(
                format!("Ciclo de import detectado: {}", module),
                Span { start: 0, end: 0 },
            );
            return false;
        }

        self.loading_modules.insert(module.to_string());

        let path = module.replace('.', "/") + ".hulk";
        let loaded = match fs::read_to_string(&path) {
            Ok(src) => match crate::parse_program(&src) {
                Ok(program) => {
                    let mut module_analyzer = SemanticAnalyzer::with_module_state(
                        self.module_cache.clone(),
                        self.namespaces.clone(),
                        self.loading_modules.clone(),
                    );
                    match module_analyzer.analyze(&program) {
                    Ok(ctx) => {
                        self.module_cache = module_analyzer.module_cache;
                        self.namespaces = module_analyzer.namespaces;
                        self.loading_modules = module_analyzer.loading_modules;
                        self.module_cache.insert(module.to_string(), ctx.clone());
                        self.namespaces
                            .insert(module.to_string(), Self::build_public_namespace(&ctx));
                        true
                    }
                    Err(diags) => {
                        self.module_cache = module_analyzer.module_cache;
                        self.namespaces = module_analyzer.namespaces;
                        self.loading_modules = module_analyzer.loading_modules;
                        for d in diags {
                            self.diagnostics.push(d);
                        }
                        false
                    }
                }
                },
                Err(diags) => {
                    for d in diags {
                        self.diagnostics.push(d);
                    }
                    false
                }
            },
            Err(_) => {
                self.diagnostics.error(
                    format!("Modulo no encontrado: {}", path),
                    Span { start: 0, end: 0 },
                );
                false
            }
        };

        self.loading_modules.remove(module);
        loaded
    }

    /// Ejecuta el analisis semantico completo sobre el programa.
    pub fn analyze(&mut self, program: &Program) -> Result<SemanticContext, Vec<Diagnostic>> {
        self.collect_top_level(program);
        self.validate_exports();
        self.validate_hierarchies();
        self.infer_program_annotations(program);

        for item in &program.items {
            self.check_item(item);
        }
        if self.diagnostics.has_errors() {
            Err(self.diagnostics.to_vec())
        } else {
            Ok(SemanticContext {
                inferred_types: self.inferred_types.clone(),
                type_shapes: self.type_shapes.clone(),
                protocol_shapes: self.protocol_shapes.clone(),
                global_symbols: self.symbols.snapshot(),
                imports: self.imports.clone(),
                exports: self.exports.clone(),
            })
        }
    }

    /// Primera pasada: registra declaraciones top-level y sus formas estructurales.
    fn collect_top_level(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::Import(imp) => {
                    self.imports.push(imp.module.clone());
                    // Eagerly load the module to collect its public symbols.
                    if self.load_module(&imp.module) {
                        // Register the module name as a namespace symbol so references to
                        // the module identifier resolve in expressions.
                        self.define_symbol(
                            &imp.module,
                            SymbolKind::Namespace,
                            SemanticType::Custom(imp.module.clone()),
                        );
                    }
                }
                Item::Export(exp) => {
                    self.exports.push(exp.module.clone());
                }
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
                    self.define_symbol(
                        &typ.name,
                        SymbolKind::Type,
                        SemanticType::Custom(typ.name.clone()),
                    );

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

    /// Valida que cada export apunte a un simbolo top-level existente.
    fn validate_exports(&mut self) {
        let mut seen = HashSet::new();
        for export_name in &self.exports {
            if !seen.insert(export_name.clone()) {
                self.diagnostics.error(
                    format!("Export duplicado: {}", export_name),
                    Span { start: 0, end: 0 },
                );
                continue;
            }

            if self.symbols.lookup(export_name).is_none() {
                self.diagnostics.error(
                    format!("Export inexistente: {}", export_name),
                    Span { start: 0, end: 0 },
                );
            }
        }
    }

    /// Construye la forma estructural de un tipo declarado.
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
            methods.insert(
                method.name.clone(),
                SemanticType::Function(params, Box::new(ret)),
            );
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

    /// Construye la forma estructural de un protocolo declarado.
    fn build_protocol_shape(&self, proto: &ProtocolDecl) -> ProtocolShape {
        let mut methods = HashMap::new();
        for method in &proto.methods {
            let params = method
                .params
                .iter()
                .map(|p| self.resolve_type_ref_silent(p.types.as_ref()))
                .collect::<Vec<_>>();
            let ret = SemanticType::from_type_ref(&method.return_type);
            methods.insert(
                method.name.clone(),
                SemanticType::Function(params, Box::new(ret)),
            );
        }

        let parent = match proto.parent.as_deref() {
            Some(TypeRef::Custom(name)) => Some(name.clone()),
            _ => None,
        };

        ProtocolShape { methods, parent }
    }

    /// Despacha el chequeo segun el tipo de item top-level.
    fn check_item(&mut self, item: &Item) {
        match item {
            Item::Import(_) => {}
            Item::Export(_) => {}
            Item::Function(func) => self.check_function_decl(func),
            Item::Type(typ) => self.check_type_decl(typ),
            Item::Protocol(proto) => self.check_protocol_decl(proto),
            Item::Macro(macr) => self.check_macro_decl(macr),
            Item::GlobalExpr(expr) => {
                self.check_expr(expr);
            }
        }
    }

    /// Valida un tipo declarado, sus campos, metodos y herencia.
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
            if let SemanticType::Custom(parent_name) =
                self.resolve_type_ref(Some(parent), self.type_decl_span(typ))
            {
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

            if let Some(parent_member) =
                self.lookup_member_type_in_parent_chain(&typ.name, &field.name)
            {
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
                if self
                    .lookup_member_type_in_parent_chain(&typ.name, &acc)
                    .is_some()
                {
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
                    let hint = format!(
                        "Declarado en span: {}..{}",
                        decl.initializer.span.start, decl.initializer.span.end
                    );
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
            let declared =
                self.resolve_type_ref(field.type_annotation.as_ref(), field.initializer.span);
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

    /// Valida un protocolo declarado y la compatibilidad con su padre.
    fn check_protocol_decl(&mut self, proto: &ProtocolDecl) {
        if let Some(parent) = &proto.parent {
            let parent_type =
                self.resolve_type_ref(Some(parent.as_ref()), self.protocol_decl_span(proto));
            let parent_ok = matches!(parent_type, SemanticType::Custom(name)
                if self.symbols.lookup(&name).map(|symbol| symbol.kind) == Some(SymbolKind::Protocol));
            if !parent_ok {
                self.diagnostics.error(
                    format!(
                        "El protocolo {} debe extender otro protocolo por nombre",
                        proto.name
                    ),
                    self.protocol_decl_span(proto),
                );
            }
        }

        if let Some(TypeRef::Custom(parent_name)) = proto.parent.as_deref() {
            if !self.protocol_conforms_to_protocol(&proto.name, parent_name) {
                self.diagnostics.error(
                    format!(
                        "El protocolo {} no es compatible con su padre {}",
                        proto.name, parent_name
                    ),
                    self.protocol_decl_span(proto),
                );
            }
        }

        if let Some(TypeRef::Custom(parent_name)) = proto.parent.as_deref() {
            if !self.protocol_conforms_to_protocol(&proto.name, parent_name) {
                self.diagnostics.error(
                    format!(
                        "El protocolo {} no es compatible con su padre {}",
                        proto.name, parent_name
                    ),
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

    /// Valida una macro declarada dentro de su propio ambito local.
    fn check_macro_decl(&mut self, macr: &MacroDecl) {
        self.enter_scope();
        for param in &macr.params {
            let typ = self.resolve_type_ref(param.type_info.as_ref(), macr.body.span);
            self.define_local(&param.name, SymbolKind::Variable, typ, macr.body.span);
        }
        self.check_expr(&macr.body);
        self.exit_scope();
    }

    /// Verifica que las cadenas de herencia referencien simbolos validos.
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
                        format!(
                            "Tipo padre no definido: {} (usado por {})",
                            link.parent, child
                        ),
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

    /// Detecta ciclos en una relacion padre-hijo ya construida.
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

    /// Obtiene un span util para diagnosticos de un tipo declarado.
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

    /// Obtiene un span util para diagnosticos de un protocolo declarado.
    fn protocol_decl_span(&self, _proto: &ProtocolDecl) -> Span {
        Span { start: 0, end: 0 }
    }

    /// Instala los simbolos integrados del lenguaje antes del chequeo del programa.
    fn install_prelude(&mut self) {
        // Builtins base para evitar falsos errores semanticos en programas validos.
        self.define_builtin_function("print", vec![SemanticType::Unknown], SemanticType::Unknown);
        self.define_builtin_function(
            "range",
            vec![SemanticType::Number, SemanticType::Number],
            SemanticType::Vector(Box::new(SemanticType::Number)),
        );
        self.define_builtin_function("sqrt", vec![SemanticType::Number], SemanticType::Number);
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

    /// Define una funcion integrada en la tabla de simbolos.
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

    /// Define un protocolo integrado en la tabla de simbolos.
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

    /// Registra un simbolo top-level y reporta redefiniciones.
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

    /// Convierte una referencia de tipo opcional sin diagnosticos adicionales.
    fn resolve_type_ref_silent(&self, type_ref: Option<&TypeRef>) -> SemanticType {
        type_ref
            .map(SemanticType::from_type_ref)
            .unwrap_or(SemanticType::Unknown)
    }
}
