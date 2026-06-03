use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicTypeEnum, FloatType, IntType, StructType};
use inkwell::types::BasicType;
use inkwell::values::{FloatValue, GlobalValue, IntValue, PointerValue};
use std::collections::HashMap;

mod expr;
mod functions;

use crate::ast::{Item, Program, TypeDecl};
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
    type_decls: HashMap<String, TypeDecl>,
    struct_types: HashMap<String, StructType<'ctx>>,
    vtable_types: HashMap<String, StructType<'ctx>>,
    vtable_globals: HashMap<String, GlobalValue<'ctx>>,
    type_ids: HashMap<String, u64>,
    method_orders: HashMap<String, Vec<String>>,
    vector_struct: StructType<'ctx>,
    current_type: Option<String>,
    current_method: Option<String>,
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
    String,
    Object,
    Vector,
}

#[derive(Clone, Copy, Debug)]
pub enum CodegenValue<'ctx> {
    Number(FloatValue<'ctx>),
    Bool(IntValue<'ctx>),
    String(PointerValue<'ctx>),
    Object(PointerValue<'ctx>),
    Vector(PointerValue<'ctx>),
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
            CodegenValue::String(_) => ValueKind::String,
            CodegenValue::Object(_) => ValueKind::Object,
            CodegenValue::Vector(_) => ValueKind::Vector,
        }
    }

    pub fn into_number(self) -> Result<FloatValue<'ctx>, String> {
        match self {
            CodegenValue::Number(value) => Ok(value),
            _ => Err("Se esperaba Number".to_string()),
        }
    }

    pub fn into_bool(self) -> Result<IntValue<'ctx>, String> {
        match self {
            CodegenValue::Bool(value) => Ok(value),
            _ => Err("Se esperaba Boolean".to_string()),
        }
    }

    pub fn into_string(self) -> Result<PointerValue<'ctx>, String> {
        match self {
            CodegenValue::String(value) => Ok(value),
            _ => Err("Se esperaba String".to_string()),
        }
    }

    pub fn into_object(self) -> Result<PointerValue<'ctx>, String> {
        match self {
            CodegenValue::Object(value) => Ok(value),
            _ => Err("Se esperaba Object".to_string()),
        }
    }

    pub fn into_vector(self) -> Result<PointerValue<'ctx>, String> {
        match self {
            CodegenValue::Vector(value) => Ok(value),
            _ => Err("Se esperaba Vector".to_string()),
        }
    }
}

impl<'ctx> CodeGenerator<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let vector_struct = context.struct_type(
            &[
                context.i64_type().into(),
                context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::default())
                    .into(),
                context.i64_type().into(),
            ],
            false,
        );

        Self {
            context,
            module: context.create_module(module_name),
            builder: context.create_builder(),
            f64_type: context.f64_type(),
            bool_type: context.bool_type(),
            scopes: vec![HashMap::new()],
            tmp_counter: 0,
            functions: HashMap::new(),
            type_decls: HashMap::new(),
            struct_types: HashMap::new(),
            vtable_types: HashMap::new(),
            vtable_globals: HashMap::new(),
            type_ids: HashMap::new(),
            method_orders: HashMap::new(),
            vector_struct,
            current_type: None,
            current_method: None,
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
        self.collect_type_decls(program);
        self.prepare_object_types(analysis)?;
        self.declare_functions(program, analysis)?;
        self.declare_methods(program, analysis)?;
        self.prepare_vtables(analysis)?;
        self.define_functions(program, analysis)?;
        self.define_methods(program, analysis)?;

        let global_exprs = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::GlobalExpr(expr) => Some(expr),
                _ => None,
            })
            .collect::<Vec<_>>();

        let fn_type = if let Some(last_expr) = global_exprs.last() {
            let value_kind = self.value_kind_for_expr(last_expr, analysis)?;
            self.basic_type_for_kind(&value_kind)?.fn_type(&[], false)
        } else {
            self.f64_type.fn_type(&[], false)
        };

        let function = self.module.add_function("main", fn_type, None);
        let block = self.context.append_basic_block(function, "entry");

        self.builder.position_at_end(block);

        if let Some((last_expr, leading_exprs)) = global_exprs.split_last() {
            for expr in leading_exprs {
                let _ = self.lower_expr(expr, analysis)?;
            }

            let value = self.lower_expr(last_expr, analysis)?;
            match value {
                CodegenValue::Number(number) => {
                    self.builder
                        .build_return(Some(&number))
                        .map_err(|e| e.to_string())?;
                }
                CodegenValue::Bool(boolean) => {
                    self.builder
                        .build_return(Some(&boolean))
                        .map_err(|e| e.to_string())?;
                }
                CodegenValue::String(string)
                | CodegenValue::Object(string)
                | CodegenValue::Vector(string) => {
                    self.builder
                        .build_return(Some(&string))
                        .map_err(|e| e.to_string())?;
                }
            }
        } else {
            let number = self.f64_type.const_float(0.0);
            self.builder
                .build_return(Some(&number))
                .map_err(|e| e.to_string())?;
        }

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

    pub(super) fn collect_type_decls(&mut self, program: &Program) {
        self.type_decls = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Type(typ) => Some((typ.name.clone(), typ.clone())),
                _ => None,
            })
            .collect();
        self.method_orders.clear();
    }

    pub(super) fn method_symbol_name(&self, type_name: &str, method_name: &str) -> String {
        format!("{}.{}", type_name, method_name)
    }

    pub(super) fn prepare_object_types(&mut self, analysis: &SemanticAnalysis) -> Result<(), String> {
        self.struct_types.clear();

        for name in self.type_decls.keys() {
            let struct_type = self.context.opaque_struct_type(&format!("obj.{}", name));
            self.struct_types.insert(name.clone(), struct_type);
        }

        for name in self.type_decls.keys() {
            let field_types = self.object_field_types(name, analysis)?;
            let mut full_types = Vec::with_capacity(field_types.len() + 1);
            full_types.push(self.vtable_ptr_type().into());
            full_types.extend(field_types);

            let struct_type = self
                .struct_types
                .get(name)
                .copied()
                .ok_or_else(|| format!("No se encontro el struct LLVM para {}", name))?;
            struct_type.set_body(&full_types, false);
        }

        Ok(())
    }

    pub(super) fn vtable_ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.context
            .i8_type()
            .ptr_type(inkwell::AddressSpace::default())
    }

    pub(super) fn prepare_vtables(&mut self, analysis: &SemanticAnalysis) -> Result<(), String> {
        self.vtable_types.clear();
        self.vtable_globals.clear();
        self.type_ids.clear();

        let mut type_names = self.type_decls.keys().cloned().collect::<Vec<_>>();
        type_names.sort();
        for (idx, name) in type_names.iter().enumerate() {
            self.type_ids.insert(name.clone(), idx as u64);
        }

        for name in type_names {
            let order = self.method_order_for_type(&name)?;
            let vtable_type = self.vtable_struct_type(&name, &order);
            let mut values = Vec::with_capacity(order.len() + 1);

            let type_id = self
                .type_ids
                .get(&name)
                .copied()
                .ok_or_else(|| format!("Type id no definido para {}", name))?;
            values.push(self.context.i64_type().const_int(type_id, false).into());

            let i8_ptr_type = self.vtable_ptr_type();
            for method_name in &order {
                let owner = self.object_method_owner(&name, method_name, analysis)?;
                let symbol = self.method_symbol_name(&owner, method_name);
                let info = self
                    .get_function(&symbol)
                    .ok_or_else(|| format!("Metodo no declarado: {}", symbol))?;
                let ptr = info
                    .function
                    .as_global_value()
                    .as_pointer_value()
                    .const_cast(i8_ptr_type);
                values.push(ptr.into());
            }

            let vtable_value = vtable_type.const_named_struct(&values);
            let global = self.module.add_global(vtable_type, None, &format!("vtable.{}", name));
            global.set_initializer(&vtable_value);
            global.set_constant(true);
            self.vtable_globals.insert(name, global);
        }

        Ok(())
    }

    pub(super) fn vtable_struct_type(
        &mut self,
        type_name: &str,
        order: &[String],
    ) -> StructType<'ctx> {
        if let Some(existing) = self.vtable_types.get(type_name).copied() {
            return existing;
        }

        let i8_ptr_type = self.vtable_ptr_type();
        let mut fields = Vec::with_capacity(order.len() + 1);
        fields.push(self.context.i64_type().into());
        for _ in 0..order.len() {
            fields.push(i8_ptr_type.into());
        }

        let struct_type = self.context.opaque_struct_type(&format!("vtable.{}", type_name));
        struct_type.set_body(&fields, false);
        self.vtable_types.insert(type_name.to_string(), struct_type);
        struct_type
    }

    pub(super) fn method_order_for_type(&mut self, type_name: &str) -> Result<Vec<String>, String> {
        if let Some(existing) = self.method_orders.get(type_name) {
            return Ok(existing.clone());
        }

        let decl = self
            .type_decls
            .get(type_name)
            .ok_or_else(|| format!("Tipo no definido: {}", type_name))?;
        let parent_name = decl.parent.as_ref().and_then(|parent| {
            if let SemanticType::Custom(name) = SemanticType::from_type_ref(parent) {
                Some(name)
            } else {
                None
            }
        });
        let methods = decl
            .methods
            .iter()
            .map(|method| method.name.clone())
            .collect::<Vec<_>>();

        let mut order = if let Some(parent) = parent_name {
            self.method_order_for_type(&parent)?
        } else {
            Vec::new()
        };

        for method_name in methods {
            if !order.iter().any(|name| name == &method_name) {
                order.push(method_name);
            }
        }

        self.method_orders.insert(type_name.to_string(), order.clone());
        Ok(order)
    }

    pub(super) fn method_slot(&mut self, type_name: &str, method_name: &str) -> Result<usize, String> {
        let order = self.method_order_for_type(type_name)?;
        order
            .iter()
            .position(|name| name == method_name)
            .ok_or_else(|| format!("El tipo {} no define el metodo {}", type_name, method_name))
    }

    pub(super) fn vtable_global(&self, type_name: &str) -> Result<GlobalValue<'ctx>, String> {
        self.vtable_globals
            .get(type_name)
            .copied()
            .ok_or_else(|| format!("Vtable no definida para {}", type_name))
    }

    pub(super) fn type_id_for(&self, type_name: &str) -> Result<u64, String> {
        self.type_ids
            .get(type_name)
            .copied()
            .ok_or_else(|| format!("Type id no definido para {}", type_name))
    }

    pub(super) fn type_is_subtype_of(
        &self,
        actual_name: &str,
        expected_name: &str,
        analysis: &SemanticAnalysis,
    ) -> bool {
        if actual_name == expected_name {
            return true;
        }

        let mut current = Some(actual_name.to_string());
        let mut visited = std::collections::HashSet::new();

        while let Some(name) = current {
            if !visited.insert(name.clone()) {
                break;
            }

            let Some(shape) = analysis.type_shapes.get(&name) else {
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

    pub(super) fn subtype_ids(
        &self,
        expected_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Result<Vec<u64>, String> {
        let mut ids = Vec::new();
        for name in self.type_decls.keys() {
            if self.type_is_subtype_of(name, expected_name, analysis) {
                ids.push(self.type_id_for(name)?);
            }
        }

        if ids.is_empty() {
            return Err(format!("No se encontraron subtipos para {}", expected_name));
        }

        Ok(ids)
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
            SemanticType::String => Ok(ValueKind::String),
            SemanticType::Custom(_) => Ok(ValueKind::Object),
            SemanticType::Vector(_) => Ok(ValueKind::Vector),
            _ => Err(format!("Tipo no soportado en codegen: {:?} -> {}", expr.id, kind)),
        }
    }

    pub(super) fn default_value_for_kind(&self, kind: ValueKind) -> Result<CodegenValue<'ctx>, String> {
        match kind {
            ValueKind::Number => Ok(CodegenValue::Number(self.f64_type.const_float(0.0))),
            ValueKind::Bool => Ok(CodegenValue::Bool(self.bool_type.const_int(0, false))),
            ValueKind::String => {
                let empty_str = self
                    .builder
                    .build_global_string_ptr("", "empty_str")
                    .unwrap()
                    .as_pointer_value();
                Ok(CodegenValue::String(empty_str))
            }
            ValueKind::Object => Ok(CodegenValue::Object(
                self.context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::default())
                    .const_null(),
            )),
            ValueKind::Vector => Ok(CodegenValue::Vector(
                self.vector_struct
                    .ptr_type(inkwell::AddressSpace::default())
                    .const_null(),
            )),
        }
    }

    pub(super) fn basic_type_for_kind(
        &self,
        kind: &ValueKind,
    ) -> Result<BasicTypeEnum<'ctx>, String> {
        match kind {
            ValueKind::Number => Ok(self.f64_type.into()),
            ValueKind::Bool => Ok(self.bool_type.into()),
            ValueKind::String | ValueKind::Object => Ok(
                self.context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::default())
                    .into(),
            ),
            ValueKind::Vector => Ok(
                self.vector_struct
                    .ptr_type(inkwell::AddressSpace::default())
                    .into(),
            ),
        }
    }

    pub(super) fn basic_type_for_semantic(
        &self,
        typ: &SemanticType,
    ) -> Result<BasicTypeEnum<'ctx>, String> {
        match typ {
            SemanticType::Number => Ok(self.f64_type.into()),
            SemanticType::Boolean => Ok(self.bool_type.into()),
            SemanticType::String => Ok(
                self.context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::default())
                    .into(),
            ),
            SemanticType::Custom(_) => Ok(
                self.context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::default())
                    .into(),
            ),
            SemanticType::Vector(_) => Ok(
                self.vector_struct
                    .ptr_type(inkwell::AddressSpace::default())
                    .into(),
            ),
            _ => Err(format!("Tipo no soportado en codegen: {}", typ)),
        }
    }

    pub(super) fn object_struct_type(&self, type_name: &str) -> Result<StructType<'ctx>, String> {
        self.struct_types
            .get(type_name)
            .copied()
            .ok_or_else(|| format!("Tipo de objeto no preparado: {}", type_name))
    }

    pub(super) fn object_field_names(&self, type_name: &str) -> Result<Vec<String>, String> {
        let decl = self
            .type_decls
            .get(type_name)
            .ok_or_else(|| format!("Tipo no definido: {}", type_name))?;

        let mut names = if let Some(parent) = &decl.parent {
            if let SemanticType::Custom(parent_name) = SemanticType::from_type_ref(parent) {
                self.object_field_names(&parent_name)?
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        names.extend(decl.fields.iter().map(|field| field.name.clone()));
        Ok(names)
    }

    pub(super) fn object_field_types(
        &self,
        type_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Result<Vec<BasicTypeEnum<'ctx>>, String> {
        let mut field_types = Vec::new();
        for field_name in self.object_field_names(type_name)? {
            let semantic_type = self.object_field_semantic_type(type_name, &field_name, analysis)?;
            field_types.push(self.basic_type_for_semantic(&semantic_type)?);
        }

        Ok(field_types)
    }

    pub(super) fn object_field_semantic_type(
        &self,
        type_name: &str,
        field_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Result<SemanticType, String> {
        let decl = self
            .type_decls
            .get(type_name)
            .ok_or_else(|| format!("Tipo no definido: {}", type_name))?;

        if let Some(shape) = analysis.type_shapes.get(type_name)
            && let Some(semantic_type) = shape.fields.get(field_name)
        {
            return Ok(semantic_type.clone());
        }

        if let Some(parent) = &decl.parent
            && let SemanticType::Custom(parent_name) = SemanticType::from_type_ref(parent)
        {
            return self.object_field_semantic_type(&parent_name, field_name, analysis);
        }

        Err(format!("No se encontro el tipo del campo {} en {}", field_name, type_name))
    }

    pub(super) fn object_method_owner(
        &self,
        type_name: &str,
        method_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Result<String, String> {
        if let Some(shape) = analysis.type_shapes.get(type_name)
            && shape.methods.contains_key(method_name)
        {
            return Ok(type_name.to_string());
        }

        let decl = self
            .type_decls
            .get(type_name)
            .ok_or_else(|| format!("Tipo no definido: {}", type_name))?;

        if let Some(parent) = &decl.parent
            && let SemanticType::Custom(parent_name) = SemanticType::from_type_ref(parent)
        {
            return self.object_method_owner(&parent_name, method_name, analysis);
        }

        Err(format!(
            "No se encontro el metodo {} en {}",
            method_name, type_name
        ))
    }

    pub(super) fn object_method_semantic_type(
        &self,
        type_name: &str,
        method_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Result<SemanticType, String> {
        let owner = self.object_method_owner(type_name, method_name, analysis)?;
        let shape = analysis
            .type_shapes
            .get(&owner)
            .ok_or_else(|| format!("Tipo no encontrado en analisis: {}", owner))?;

        shape
            .methods
            .get(method_name)
            .cloned()
            .ok_or_else(|| format!("No se encontro el metodo {} en {}", method_name, owner))
    }

    pub(super) fn alloca_for_kind(
        &self,
        kind: &ValueKind,
        name: &str,
    ) -> Result<PointerValue<'ctx>, String> {
        let ty = self.basic_type_for_kind(kind)?;
        self.builder.build_alloca(ty, name).map_err(|e| e.to_string())
    }

    pub(super) fn load_value(
        &self,
        kind: &ValueKind,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<CodegenValue<'ctx>, String> {
        match kind {
            ValueKind::Number => Ok(CodegenValue::Number(
                self.builder
                    .build_load(self.f64_type, ptr, name)
                    .map_err(|e| e.to_string())?
                    .into_float_value(),
            )),
            ValueKind::Bool => Ok(CodegenValue::Bool(
                self.builder
                    .build_load(self.bool_type, ptr, name)
                    .map_err(|e| e.to_string())?
                    .into_int_value(),
            )),
            ValueKind::String => {
                let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                Ok(CodegenValue::String(
                    self.builder
                        .build_load(i8_ptr_type, ptr, name)
                        .map_err(|e| e.to_string())?
                        .into_pointer_value(),
                ))
            }
            ValueKind::Object => {
                let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                Ok(CodegenValue::Object(
                    self.builder
                        .build_load(i8_ptr_type, ptr, name)
                        .map_err(|e| e.to_string())?
                        .into_pointer_value(),
                ))
            }
            ValueKind::Vector => {
                let vec_ptr_type = self
                    .vector_struct
                    .ptr_type(inkwell::AddressSpace::default());
                Ok(CodegenValue::Vector(
                    self.builder
                        .build_load(vec_ptr_type, ptr, name)
                        .map_err(|e| e.to_string())?
                        .into_pointer_value(),
                ))
            }
        }
    }

    pub(super) fn store_value(
        &self,
        ptr: PointerValue<'ctx>,
        value: CodegenValue<'ctx>,
    ) -> Result<(), String> {
        match value {
            CodegenValue::Number(number) => {
                self.builder
                    .build_store(ptr, number)
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            CodegenValue::Bool(boolean) => {
                self.builder
                    .build_store(ptr, boolean)
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            CodegenValue::String(string) | CodegenValue::Object(string) => {
                self.builder
                    .build_store(ptr, string)
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            CodegenValue::Vector(vector) => {
                self.builder
                    .build_store(ptr, vector)
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
        }
    }

    pub(super) fn value_kind_from_semantic(
        &self,
        typ: &SemanticType,
    ) -> Result<ValueKind, String> {
        match typ {
            SemanticType::Number => Ok(ValueKind::Number),
            SemanticType::Boolean => Ok(ValueKind::Bool),
            SemanticType::String => Ok(ValueKind::String),
            SemanticType::Custom(_) => Ok(ValueKind::Object),
            SemanticType::Vector(_) => Ok(ValueKind::Vector),
            _ => Err(format!("Tipo no soportado en codegen: {}", typ)),
        }
    }
}
