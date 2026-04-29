use std::collections::HashMap;

use crate::types::SemanticType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Function,
    Type,
    Protocol,
    Macro,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub typ: SemanticType,
}

#[derive(Debug, Default)]
pub struct SymbolTable {
    scopes: Vec<HashMap<String, Symbol>>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn define(&mut self, symbol: Symbol) -> bool {
        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(&symbol.name) {
                return false;
            }
            scope.insert(symbol.name.clone(), symbol);
            return true;
        }

        false
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
    }

    pub fn update_type(&mut self, name: &str, typ: SemanticType) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(symbol) = scope.get_mut(name) {
                symbol.typ = typ;
                return true;
            }
        }

        false
    }
}
