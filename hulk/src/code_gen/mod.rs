use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{FloatType, IntType};
use inkwell::values::{FloatValue, IntValue, PointerValue};
use std::collections::HashMap;

mod expr;
mod functions;

use crate::ast::{Item, Program};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

pub struct CodeGenerator<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    f64_type: FloatType<'ctx>,
    bool_type: IntType<'ctx>,
    scopes: Vec<HashMap<String, VarInfo<'ctx>>>,
    tmp_counter: u64,
    functions: HashMap<String, FunctionInfo<'ctx>>,
}

#[derive(Clone, Debug)]
pub struct FunctionInfo<'ctx> {
    pub function: inkwell::values::FunctionValue<'ctx>,
    pub params: Vec<ValueKind>,
    pub ret: ValueKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Number,
    Bool,
}

#[derive(Clone, Copy, Debug)]
pub enum CodegenValue<'ctx> {
    Number(FloatValue<'ctx>),
    Bool(IntValue<'ctx>),
}

#[derive(Clone, Copy, Debug)]
pub struct VarInfo<'ctx> {
    pub ptr: PointerValue<'ctx>,
    pub kind: ValueKind,
}

impl<'ctx> CodegenValue<'ctx> {
    pub fn kind(&self) -> ValueKind {
        match self {
            CodegenValue::Number(_) => ValueKind::Number,
            CodegenValue::Bool(_) => ValueKind::Bool,
        }
    }

    pub fn into_number(self) -> Result<FloatValue<'ctx>, String> {
        match self {
            CodegenValue::Number(value) => Ok(value),
            CodegenValue::Bool(_) => Err("Se esperaba Number".to_string()),
        }
    }

    pub fn into_bool(self) -> Result<IntValue<'ctx>, String> {
        match self {
            CodegenValue::Bool(value) => Ok(value),
            CodegenValue::Number(_) => Err("Se esperaba Boolean".to_string()),
        }
    }
}

impl<'ctx> CodeGenerator<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        Self {
            context,
            module: context.create_module(module_name),
            builder: context.create_builder(),
            f64_type: context.f64_type(),
            bool_type: context.bool_type(),
            scopes: vec![HashMap::new()],
            tmp_counter: 0,
            functions: HashMap::new(),
        }
    }

    pub fn module(&self) -> &Module<'ctx> {
        &self.module
    }

    pub fn codegen_program(
        &mut self,
        program: &Program,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
        self.declare_functions(program, analysis)?;
        self.define_functions(program, analysis)?;

        let expr = program
            .items
            .iter()
            .find_map(|item| match item {
                Item::GlobalExpr(expr) => Some(expr),
                _ => None,
            })
            .ok_or_else(|| "No hay expresion global para compilar".to_string())?;

        let fn_type = self.f64_type.fn_type(&[], false);
        let function = self.module.add_function("main", fn_type, None);
        let block = self.context.append_basic_block(function, "entry");

        self.builder.position_at_end(block);

        let value = self.lower_expr(expr, analysis)?;
        let number = value.into_number()?;
        self.builder
            .build_return(Some(&number))
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub(super) fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(super) fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub(super) fn insert_var(&mut self, name: String, info: VarInfo<'ctx>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, info);
        }
    }

    pub(super) fn lookup_var(&self, name: &str) -> Option<VarInfo<'ctx>> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(*info);
            }
        }

        None
    }

    pub(super) fn fresh_tmp(&mut self, prefix: &str) -> String {
        let name = format!("{}_{}", prefix, self.tmp_counter);
        self.tmp_counter += 1;
        name
    }

    pub(super) fn value_kind_for_expr(
        &self,
        expr: &crate::ast::Expr,
        analysis: &SemanticAnalysis,
    ) -> Result<ValueKind, String> {
        let kind = analysis
            .inferred_types
            .get(&expr.id)
            .ok_or_else(|| "No se encontro tipo inferido".to_string())?;

        match kind {
            SemanticType::Number => Ok(ValueKind::Number),
            SemanticType::Boolean => Ok(ValueKind::Bool),
            _ => Err("Tipo no soportado en codegen".to_string()),
        }
    }

    pub(super) fn default_value_for_kind(&self, kind: ValueKind) -> CodegenValue<'ctx> {
        match kind {
            ValueKind::Number => CodegenValue::Number(self.f64_type.const_float(0.0)),
            ValueKind::Bool => CodegenValue::Bool(self.bool_type.const_int(0, false)),
        }
    }

    pub(super) fn value_kind_from_semantic(
        &self,
        typ: &SemanticType,
    ) -> Result<ValueKind, String> {
        match typ {
            SemanticType::Number => Ok(ValueKind::Number),
            SemanticType::Boolean => Ok(ValueKind::Bool),
            _ => Err("Tipo no soportado en codegen".to_string()),
        }
    }
}
