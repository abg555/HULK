use std::collections::HashMap;

use crate::ast::Span;
use crate::semantic::symbol_table::{Symbol, SymbolKind};
use crate::semantic::types::SemanticType;

use super::SemanticAnalyzer;

impl SemanticAnalyzer {
    /// Entra en un nuevo ambito de simbolos y de asignacion definitiva.
    pub(super) fn enter_scope(&mut self) {
        self.symbols.enter_scope();
        self.assigned_scopes.push(HashMap::new());
        self.readonly_scopes.push(HashMap::new());
    }

    /// Sale del ambito actual si existe uno externo al global.
    pub(super) fn exit_scope(&mut self) {
        self.symbols.exit_scope();
        if self.assigned_scopes.len() > 1 {
            self.assigned_scopes.pop();
        }
        if self.readonly_scopes.len() > 1 {
            self.readonly_scopes.pop();
        }
    }

    /// Indica si una variable fue asignada en todos los caminos conocidos.
    pub(super) fn is_definitely_assigned(&self, name: &str) -> bool {
        self.assigned_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
            .unwrap_or(true)
    }

    /// Marca una variable como asignada en el ambito donde fue declarada.
    pub(super) fn mark_assigned(&mut self, name: &str) {
        for scope in self.assigned_scopes.iter_mut().rev() {
            if let Some(state) = scope.get_mut(name) {
                *state = true;
                return;
            }
        }
    }

    /// Registra una variable local de solo lectura con una razon explicativa.
    pub(super) fn mark_local_readonly(&mut self, name: &str, reason: impl Into<String>) {
        if let Some(scope) = self.readonly_scopes.last_mut() {
            scope.insert(name.to_string(), reason.into());
        }
    }

    /// Devuelve la razon de solo lectura asociada a un nombre, si existe.
    pub(super) fn readonly_reason(&self, name: &str) -> Option<&str> {
        self.readonly_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).map(String::as_str))
    }

    /// Define una variable local ya inicializada.
    pub(super) fn define_local(
        &mut self,
        name: &str,
        kind: SymbolKind,
        typ: SemanticType,
        span: Span,
    ) {
        self.define_local_with_state(name, kind, typ, span, true);
    }

    /// Define una variable local permitiendo indicar su estado de asignacion inicial.
    pub(super) fn define_local_with_state(
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
                format!(
                    "Sombra de simbolo: '{}' oculta un simbolo externo de tipo {:?}",
                    name, existing.kind
                ),
                span,
            );
        }

        let symbol = Symbol {
            name: name.to_string(),
            kind,
            typ,
        };

        if !self.symbols.define(symbol) {
            self.diagnostics
                .error(format!("Redefinicion de simbolo local: {}", name), span);
            return;
        }

        if kind == SymbolKind::Variable
            && let Some(current_scope) = self.assigned_scopes.last_mut()
        {
            current_scope.insert(name.to_string(), is_assigned);
        }
    }

    /// Fusiona estados de asignacion tras un if o una bifurcacion multiple.
    pub(super) fn merge_definite_assignment_states(
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

    /// Intersecta dos estados de asignacion para modelar ciclos de manera conservadora.
    pub(super) fn intersect_definite_assignment_states(
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
}
