use std::collections::{HashMap, HashSet};

use crate::ast::{FunctionDecl, Span, TypeDecl, TypeRef};
use crate::semantic::symbol_table::SymbolKind;
use crate::semantic::types::SemanticType;

use super::SemanticAnalyzer;

impl SemanticAnalyzer {
    /// Decide si un tipo actual es compatible con el tipo esperado.
    pub(super) fn is_compatible_type(
        &self,
        expected: &SemanticType,
        actual: &SemanticType,
    ) -> bool {
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
            ) => self.function_signature_compatible(
                expected_params,
                expected_ret,
                actual_params,
                actual_ret,
            ),
            (SemanticType::Custom(expected_name), SemanticType::Function(_, _)) => {
                self.function_compatible_with_functor_protocol(expected_name, actual)
            }
            (SemanticType::Function(_, _), SemanticType::Custom(actual_name)) => {
                self.functor_compatible_with_function(expected, actual_name)
            }
            (SemanticType::Custom(expected_name), SemanticType::Custom(actual_name)) => {
                self.custom_type_compatible(expected_name, actual_name)
            }
            _ => false,
        }
    }

    /// Comprueba compatibilidad entre firmas de funcion.
    pub(super) fn function_signature_compatible(
        &self,
        expected_params: &[SemanticType],
        expected_ret: &SemanticType,
        actual_params: &[SemanticType],
        actual_ret: &SemanticType,
    ) -> bool {
        expected_params.len() == actual_params.len()
            && expected_params.iter().zip(actual_params.iter()).all(
                |(expected_param, actual_param)| {
                    self.is_compatible_type(actual_param, expected_param)
                },
            )
            && self.is_compatible_type(expected_ret, actual_ret)
    }

    /// Busca la firma `invoke` que hace invocable a un tipo o protocolo.
    pub(super) fn lookup_functor_invoke_type(&self, type_name: &str) -> Option<SemanticType> {
        let symbol = self.symbols.lookup(type_name)?;
        match symbol.kind {
            SymbolKind::Protocol => self.lookup_protocol_member_type(type_name, "invoke"),
            SymbolKind::Type => self.lookup_member_type(type_name, "invoke"),
            _ => None,
        }
    }

    /// Permite usar una funcion/lambda donde se espera un protocolo functor.
    fn function_compatible_with_functor_protocol(
        &self,
        expected_name: &str,
        actual: &SemanticType,
    ) -> bool {
        let Some(symbol) = self.symbols.lookup(expected_name) else {
            return false;
        };
        if symbol.kind != SymbolKind::Protocol {
            return false;
        }

        let Some(invoke_signature) = self.lookup_protocol_member_type(expected_name, "invoke")
        else {
            return false;
        };

        self.is_compatible_type(&invoke_signature, actual)
    }

    /// Permite usar un objeto con `invoke` donde se espera un tipo funcion.
    fn functor_compatible_with_function(&self, expected: &SemanticType, actual_name: &str) -> bool {
        let Some(invoke_signature) = self.lookup_functor_invoke_type(actual_name) else {
            return false;
        };

        self.is_compatible_type(expected, &invoke_signature)
    }

    /// Comprueba compatibilidad entre tipos personalizados, tipos y protocolos.
    pub(super) fn custom_type_compatible(&self, expected_name: &str, actual_name: &str) -> bool {
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
            (SymbolKind::Namespace, SymbolKind::Namespace) => expected_name == actual_name,
            (SymbolKind::Namespace, _) | (_, SymbolKind::Namespace) => false,
            (SymbolKind::Type, SymbolKind::Type) => {
                self.type_is_subtype_of(actual_name, expected_name)
            }
            (SymbolKind::Protocol, SymbolKind::Type) => {
                self.type_conforms_to_protocol(actual_name, expected_name)
            }
            (SymbolKind::Protocol, SymbolKind::Protocol) => {
                self.protocol_conforms_to_protocol(actual_name, expected_name)
            }
            _ => false,
        }
    }

    /// Recorre la cadena de herencia para validar subtipado estructural.
    pub(super) fn type_is_subtype_of(&self, actual_name: &str, expected_name: &str) -> bool {
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

    /// Comprueba que un tipo implemente todos los metodos de un protocolo.
    pub(super) fn type_conforms_to_protocol(&self, type_name: &str, protocol_name: &str) -> bool {
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

            if !expected_params.iter().zip(actual_params.iter()).all(
                |(expected_param, actual_param)| {
                    self.is_compatible_type(actual_param, expected_param)
                },
            ) {
                return false;
            }

            if !self.is_compatible_type(expected_ret, actual_ret) {
                return false;
            }
        }

        true
    }

    /// Comprueba que un protocolo sea compatible o extienda al protocolo esperado.
    pub(super) fn protocol_conforms_to_protocol(
        &self,
        actual_name: &str,
        expected_name: &str,
    ) -> bool {
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

            if !expected_params.iter().zip(actual_params.iter()).all(
                |(expected_param, actual_param)| {
                    self.is_compatible_type(actual_param, expected_param)
                },
            ) {
                return false;
            }

            if !self.is_compatible_type(expected_ret, actual_ret) {
                return false;
            }
        }

        true
    }

    /// Determina si un protocolo extiende transitivamente a otro.
    pub(super) fn protocol_extends(&self, actual_name: &str, expected_name: &str) -> bool {
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

    /// Reune los metodos visibles de un protocolo, incluyendo sus ancestros.
    pub(super) fn collect_protocol_methods(
        &self,
        protocol_name: &str,
    ) -> Option<HashMap<String, SemanticType>> {
        let mut collected = HashMap::new();
        self.collect_protocol_methods_recursive(protocol_name, &mut collected)?;
        Some(collected)
    }

    /// Version recursiva de recoleccion de metodos de protocolo.
    pub(super) fn collect_protocol_methods_recursive(
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

    /// Resuelve el tipo de un acceso a miembro y emite diagnosticos si falla.
    pub(super) fn resolve_member_type(
        &mut self,
        object_type: &SemanticType,
        member: &str,
        span: Span,
    ) -> SemanticType {
        if matches!(object_type, SemanticType::Function(_, _)) && member == "invoke" {
            return object_type.clone();
        }

        let SemanticType::Custom(type_name) = object_type else {
            self.diagnostics.error(
                format!(
                    "Acceso a miembro {} sobre valor no estructurado ({})",
                    member, object_type
                ),
                span,
            );
            return SemanticType::Unknown;
        };

        // If the object is a loaded namespace (module), look for the member in the
        // module's public symbol table first.
        if let Some(ns) = self.namespaces.get(type_name) {
            if let Some(sym) = ns.get(member) {
                return sym.typ.clone();
            } else {
                self.diagnostics.error(
                    format!("El modulo {} no define el miembro {}", type_name, member),
                    span,
                );
                return SemanticType::Unknown;
            }
        }

        if let Some(member_type) = self.lookup_member_type(type_name, member) {
            return member_type;
        }

        self.diagnostics.error(
            format!("El tipo {} no define el miembro {}", type_name, member),
            span,
        );
        SemanticType::Unknown
    }

    /// Busca un miembro en un tipo concreto y en su cadena de herencia.
    pub(super) fn lookup_member_type(&self, type_name: &str, member: &str) -> Option<SemanticType> {
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

    /// Busca un miembro solo en los ancestros del tipo.
    pub(super) fn lookup_member_type_in_parent_chain(
        &self,
        type_name: &str,
        member: &str,
    ) -> Option<SemanticType> {
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

    /// Busca la firma de un metodo en un protocolo y sus padres.
    pub(super) fn lookup_protocol_member_type(
        &self,
        protocol_name: &str,
        member: &str,
    ) -> Option<SemanticType> {
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

    /// Valida que la firma de un metodo sobreescrito sea compatible con la del padre.
    pub(super) fn validate_method_override(&mut self, typ: &TypeDecl, method: &FunctionDecl) {
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

        if let Some(parent_signature) =
            self.lookup_member_type_in_parent_chain(&typ.name, &method.name)
        {
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

    /// Valida que los argumentos del constructor del padre coincidan con su forma esperada.
    pub(super) fn validate_parent_constructor_args(
        &mut self,
        typ: &TypeDecl,
        parent_name: &str,
        parent_shape: &super::TypeShape,
    ) {
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

        for (idx, (expected, arg_expr)) in parent_shape
            .ctor_params
            .iter()
            .zip(typ.parent_arg.iter())
            .enumerate()
        {
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

    /// Resuelve una referencia de tipo y reporta errores si no existe.
    pub(super) fn resolve_type_ref(
        &mut self,
        type_ref: Option<&TypeRef>,
        span: Span,
    ) -> SemanticType {
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

    /// Obtiene el tipo de un destino de asignacion valido.
    pub(super) fn check_assignment_target(&mut self, target: &crate::ast::Expr) -> SemanticType {
        if let crate::ast::KindExpr::Variable(var) = &target.kind {
            if let Some(symbol) = self.symbols.lookup(&var.name) {
                return symbol.typ.clone();
            }

            self.diagnostics.error(
                format!("Identificador no definido: {}", var.name),
                target.span,
            );
            return SemanticType::Unknown;
        }

        self.check_expr(target)
    }

    /// Comprueba un operador binario y devuelve el tipo resultante.
    pub(super) fn check_binary(
        &mut self,
        span: Span,
        operator: &crate::ast::BinaryOperator,
        left: SemanticType,
        right: SemanticType,
    ) -> SemanticType {
        use crate::ast::BinaryOperator::*;

        match operator {
            Add | Sub | Mul | Div | Pow | Mod => {
                self.expect_type(span, &left, &SemanticType::Number, "operador aritmetico");
                self.expect_type(span, &right, &SemanticType::Number, "operador aritmetico");
                SemanticType::Number
            }
            And | Or => {
                self.expect_type(span, &left, &SemanticType::Boolean, "operador logico");
                self.expect_type(span, &right, &SemanticType::Boolean, "operador logico");
                SemanticType::Boolean
            }
            Equal | NotEqual | Less | Greater | LessEqual | GreaterEqual => {
                if !self.is_compatible_type(&left, &right)
                    && !self.is_compatible_type(&right, &left)
                {
                    self.diagnostics.error(
                        format!("Comparacion incompatible entre {} y {}", left, right),
                        span,
                    );
                }
                SemanticType::Boolean
            }
            Concat | FullConcat => SemanticType::String,
        }
    }

    /// Verifica que un valor tenga el tipo esperado y, si no, emite un diagnostico.
    pub(super) fn expect_type(
        &mut self,
        span: Span,
        actual: &SemanticType,
        expected: &SemanticType,
        ctx: &str,
    ) {
        if !expected.is_assignable_from(actual) {
            self.diagnostics.error(
                format!(
                    "Tipo incompatible en {}: se esperaba {}, se obtuvo {}",
                    ctx, expected, actual
                ),
                span,
            );
        }
    }
}
