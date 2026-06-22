use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicType;
use inkwell::types::{BasicTypeEnum, FloatType, IntType, StructType};
use inkwell::values::{FloatValue, GlobalValue, IntValue, PointerValue};
use std::collections::{HashMap, HashSet};
use std::fs;

mod expr;
mod functions;

use crate::ast::{Item, Program, TypeDecl};
use crate::semantic::SemanticAnalysis;
use crate::semantic::SemanticAnalyzer;
use crate::semantic::types::SemanticType;

#[derive(Clone)]
pub struct ImportedModuleUnit {
    pub module: String,
    pub program: Program,
    pub analysis: SemanticAnalysis,
}

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
    closure_struct: StructType<'ctx>,
    thunk_functions: HashMap<String, inkwell::values::FunctionValue<'ctx>>,
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
    Closure,
}

#[derive(Clone, Copy, Debug)]
pub enum CodegenValue<'ctx> {
    Number(FloatValue<'ctx>),
    Bool(IntValue<'ctx>),
    String(PointerValue<'ctx>),
    Object(PointerValue<'ctx>),
    Vector(PointerValue<'ctx>),
    Closure(PointerValue<'ctx>),
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
            CodegenValue::Closure(_) => ValueKind::Closure,
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

    pub fn into_closure(self) -> Result<PointerValue<'ctx>, String> {
        match self {
            CodegenValue::Closure(value) => Ok(value),
            _ => Err("Se esperaba un valor funcion (closure)".to_string()),
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

        let i8_ptr_type = context.i8_type().ptr_type(inkwell::AddressSpace::default());
        let closure_struct = context.struct_type(&[i8_ptr_type.into(), i8_ptr_type.into()], false);

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
            closure_struct,
            thunk_functions: HashMap::new(),
            current_type: None,
            current_method: None,
        }
    }

    pub fn with_source_dir(
        context: &'ctx Context,
        module_name: &str,
        _source_path: &std::path::Path,
    ) -> Self {
        // Currently just delegates to new() - source_path could be used for debug info in future
        Self::new(context, module_name)
    }

    pub fn module(&self) -> &Module<'ctx> {
        &self.module
    }

    pub fn codegen_program(
        &mut self,
        program: &Program,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
        let imported_units = self.collect_imported_module_units(program)?;

        self.collect_type_decls(program);
        self.prepare_object_types(analysis)?;
        self.declare_functions(program, analysis)?;
        for unit in &imported_units {
            self.declare_functions(&unit.program, &unit.analysis)?;
        }
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

        let fn_type = self.context.i32_type().fn_type(&[], false);
        let function = self.module.add_function("main", fn_type, None);
        let block = self.context.append_basic_block(function, "entry");

        self.builder.position_at_end(block);

        if let Some((last_expr, leading_exprs)) = global_exprs.split_last() {
            for expr in leading_exprs {
                let _ = self.lower_expr(expr, analysis)?;
            }

            let _ = self.lower_expr(last_expr, analysis)?;
        }

        let zero = self.context.i32_type().const_int(0, false);
        self.builder
            .build_return(Some(&zero))
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn collect_imported_module_units(
        &self,
        program: &Program,
    ) -> Result<Vec<ImportedModuleUnit>, String> {
        let mut units = Vec::new();
        let mut visited = HashSet::new();

        for item in &program.items {
            let Item::Import(import_decl) = item else {
                continue;
            };

            self.load_import_unit_recursive(&import_decl.module, &mut visited, &mut units)?;
        }

        Ok(units)
    }

    fn load_import_unit_recursive(
        &self,
        module: &str,
        visited: &mut HashSet<String>,
        units: &mut Vec<ImportedModuleUnit>,
    ) -> Result<(), String> {
        if !visited.insert(module.to_string()) {
            return Ok(());
        }

        let path = format!("{}.hulk", module.replace('.', "/"));
        let source = fs::read_to_string(&path).map_err(|e| {
            format!(
                "No se pudo leer modulo importado {} ({}): {}",
                module, path, e
            )
        })?;
        let program = crate::parse_program(&source).map_err(|diags| {
            format!(
                "No se pudo parsear modulo importado {}: {:?}",
                module, diags
            )
        })?;
        let analysis = SemanticAnalyzer::new().analyze(&program).map_err(|diags| {
            format!(
                "No se pudo analizar modulo importado {}: {:?}",
                module, diags
            )
        })?;

        for item in &program.items {
            let Item::Import(import_decl) = item else {
                continue;
            };

            self.load_import_unit_recursive(&import_decl.module, visited, units)?;
        }

        units.push(ImportedModuleUnit {
            module: module.to_string(),
            program,
            analysis,
        });

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

    pub(super) fn prepare_object_types(
        &mut self,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
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
            let global = self
                .module
                .add_global(vtable_type, None, &format!("vtable.{}", name));
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

        let struct_type = self
            .context
            .opaque_struct_type(&format!("vtable.{}", type_name));
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

        self.method_orders
            .insert(type_name.to_string(), order.clone());
        Ok(order)
    }

    pub(super) fn method_slot(
        &mut self,
        type_name: &str,
        method_name: &str,
    ) -> Result<usize, String> {
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
            SemanticType::Function(_, _) => Ok(ValueKind::Closure),
            _ => Err(format!(
                "Tipo no soportado en codegen: {:?} -> {}",
                expr.id, kind
            )),
        }
    }

    pub(super) fn default_value_for_kind(
        &self,
        kind: ValueKind,
    ) -> Result<CodegenValue<'ctx>, String> {
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
            ValueKind::Closure => Ok(CodegenValue::Closure(
                self.closure_struct
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
            ValueKind::String | ValueKind::Object => Ok(self
                .context
                .i8_type()
                .ptr_type(inkwell::AddressSpace::default())
                .into()),
            ValueKind::Vector => Ok(self
                .vector_struct
                .ptr_type(inkwell::AddressSpace::default())
                .into()),
            ValueKind::Closure => Ok(self
                .closure_struct
                .ptr_type(inkwell::AddressSpace::default())
                .into()),
        }
    }

    pub(super) fn basic_type_for_semantic(
        &self,
        typ: &SemanticType,
    ) -> Result<BasicTypeEnum<'ctx>, String> {
        match typ {
            SemanticType::Number => Ok(self.f64_type.into()),
            SemanticType::Boolean => Ok(self.bool_type.into()),
            SemanticType::String => Ok(self
                .context
                .i8_type()
                .ptr_type(inkwell::AddressSpace::default())
                .into()),
            SemanticType::Custom(_) => Ok(self
                .context
                .i8_type()
                .ptr_type(inkwell::AddressSpace::default())
                .into()),
            SemanticType::Vector(_) => Ok(self
                .vector_struct
                .ptr_type(inkwell::AddressSpace::default())
                .into()),
            SemanticType::Function(_, _) => Ok(self
                .closure_struct
                .ptr_type(inkwell::AddressSpace::default())
                .into()),
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
            let semantic_type =
                self.object_field_semantic_type(type_name, &field_name, analysis)?;
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

        Err(format!(
            "No se encontro el tipo del campo {} en {}",
            field_name, type_name
        ))
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
        self.builder
            .build_alloca(ty, name)
            .map_err(|e| e.to_string())
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
                let i8_ptr_type = self
                    .context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::default());
                Ok(CodegenValue::String(
                    self.builder
                        .build_load(i8_ptr_type, ptr, name)
                        .map_err(|e| e.to_string())?
                        .into_pointer_value(),
                ))
            }
            ValueKind::Object => {
                let i8_ptr_type = self
                    .context
                    .i8_type()
                    .ptr_type(inkwell::AddressSpace::default());
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
            ValueKind::Closure => {
                let closure_ptr_type = self
                    .closure_struct
                    .ptr_type(inkwell::AddressSpace::default());
                Ok(CodegenValue::Closure(
                    self.builder
                        .build_load(closure_ptr_type, ptr, name)
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
            CodegenValue::Closure(closure) => {
                self.builder
                    .build_store(ptr, closure)
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
        }
    }

    pub(super) fn value_kind_from_semantic(&self, typ: &SemanticType) -> Result<ValueKind, String> {
        match typ {
            SemanticType::Number => Ok(ValueKind::Number),
            SemanticType::Boolean => Ok(ValueKind::Bool),
            SemanticType::String => Ok(ValueKind::String),
            SemanticType::Custom(_) => Ok(ValueKind::Object),
            SemanticType::Vector(_) => Ok(ValueKind::Vector),
            SemanticType::Function(_, _) => Ok(ValueKind::Closure),
            _ => Err(format!("Tipo no soportado en codegen: {}", typ)),
        }
    }

    /// Si `name` es un protocolo "functor" (un unico metodo `invoke` con firma
    /// de funcion), devuelve sus parametros y tipo de retorno. Esto permite
    /// tratar valores de ese tipo de protocolo como closures en codegen,
    /// reusando la misma representacion runtime que las lambdas.
    pub(super) fn functor_protocol_signature(
        &self,
        name: &str,
        analysis: &SemanticAnalysis,
    ) -> Option<(Vec<SemanticType>, SemanticType)> {
        let shape = analysis.protocol_shapes.get(name)?;
        if shape.methods.len() != 1 {
            return None;
        }
        let SemanticType::Function(params, ret) = shape.methods.get("invoke")? else {
            return None;
        };
        Some((params.clone(), (**ret).clone()))
    }

    /// Como `value_kind_from_semantic`, pero ademas reconoce protocolos
    /// functor declarados explicitamente (`protocol P { invoke(...): T; }`)
    /// y los representa como `ValueKind::Closure`.
    pub(super) fn value_kind_from_declared_type(
        &self,
        typ: &SemanticType,
        analysis: &SemanticAnalysis,
    ) -> Result<ValueKind, String> {
        if let SemanticType::Custom(name) = typ {
            if self.functor_protocol_signature(name, analysis).is_some() {
                return Ok(ValueKind::Closure);
            }
        }
        self.value_kind_from_semantic(typ)
    }

    pub(super) fn is_protocol_name(&self, name: &str, analysis: &SemanticAnalysis) -> bool {
        analysis.protocol_shapes.contains_key(name)
    }

    pub(super) fn types_conforming_to_protocol(
        &self,
        protocol_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Vec<String> {
        let Some(proto_shape) = analysis.protocol_shapes.get(protocol_name) else {
            return Vec::new();
        };

        let mut result: Vec<String> = self
            .type_decls
            .keys()
            .filter(|type_name| {
                let Some(type_shape) = analysis.type_shapes.get(*type_name) else {
                    return false;
                };
                proto_shape
                    .methods
                    .keys()
                    .all(|m| type_shape.methods.contains_key(m))
            })
            .cloned()
            .collect();

        result.sort();
        result
    }

    pub(super) fn protocol_method_return_kind(
        &self,
        protocol_name: &str,
        method_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Result<ValueKind, String> {
        let shape = analysis
            .protocol_shapes
            .get(protocol_name)
            .ok_or_else(|| format!("Protocolo no encontrado: {}", protocol_name))?;
        let method_type = shape
            .methods
            .get(method_name)
            .ok_or_else(|| format!("Metodo {} no en protocolo {}", method_name, protocol_name))?;
        match method_type {
            SemanticType::Function(_, ret) => self.value_kind_from_semantic(ret),
            other => Err(format!(
                "Tipo de metodo invalido en protocolo {}: {}",
                protocol_name, other
            )),
        }
    }

    /// Determine an effective runtime return kind for a protocol method.
    /// If all concrete types implementing the protocol return the same
    /// `ValueKind` for `method_name`, that kind is returned. Otherwise we
    /// fall back to the protocol-declared return kind.
    pub(super) fn effective_protocol_method_return_kind(
        &self,
        protocol_name: &str,
        method_name: &str,
        analysis: &SemanticAnalysis,
    ) -> Result<ValueKind, String> {
        let proto_ret = self.protocol_method_return_kind(protocol_name, method_name, analysis)?;
        let conforming = self.types_conforming_to_protocol(protocol_name, analysis);
        if conforming.is_empty() {
            return Ok(proto_ret);
        }

        let mut concrete_ret: Option<ValueKind> = None;
        for type_name in &conforming {
            let owner = match self.object_method_owner(type_name, method_name, analysis) {
                Ok(o) => o,
                Err(_) => return Ok(proto_ret),
            };
            let symbol = self.method_symbol_name(&owner, method_name);
            let info = match self.get_function(&symbol) {
                Some(i) => i,
                None => return Ok(proto_ret),
            };
            if let Some(existing) = concrete_ret {
                if existing != info.ret {
                    return Ok(proto_ret);
                }
            } else {
                concrete_ret = Some(info.ret);
            }
        }

        Ok(concrete_ret.unwrap_or(proto_ret))
    }

    /// Dispatch vtable generico: emite una llamada indirecta via vtable al metodo
    /// `method_name` del tipo concreto `type_name`, pasando `receiver` como self
    /// y `extra_args` como argumentos adicionales.
    pub(super) fn emit_vtable_call(
        &mut self,
        receiver: PointerValue<'ctx>,
        type_name: &str,
        method_name: &str,
        info: &FunctionInfo<'ctx>,
        extra_args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
    ) -> Result<CodegenValue<'ctx>, String> {
        use inkwell::AddressSpace;

        let object_struct = self.object_struct_type(type_name)?;
        let typed_ptr = self.cast_object_ptr(&receiver, type_name)?;

        let vtable_ptr_slot = self
            .builder
            .build_struct_gep(object_struct, typed_ptr, 0, "vtable_slot")
            .map_err(|e| e.to_string())?;
        let vtable_ptr = self
            .builder
            .build_load(
                self.context.i8_type().ptr_type(AddressSpace::default()),
                vtable_ptr_slot,
                "vtable_load",
            )
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let slot = self.method_slot(type_name, method_name)? + 1;
        let order = self.method_order_for_type(type_name)?;
        let vtable_type = self.vtable_struct_type(type_name, &order);
        let vtable_typed = self
            .builder
            .build_pointer_cast(
                vtable_ptr,
                vtable_type.ptr_type(AddressSpace::default()),
                "vtable_typed",
            )
            .map_err(|e| e.to_string())?;
        let method_ptr_slot = self
            .builder
            .build_struct_gep(vtable_type, vtable_typed, slot as u32, "method_slot")
            .map_err(|e| e.to_string())?;
        let method_ptr = self
            .builder
            .build_load(
                self.context.i8_type().ptr_type(AddressSpace::default()),
                method_ptr_slot,
                "method_load",
            )
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        let fn_type = self.fn_type_for_signature(&info.params, info.ret);
        let fn_ptr = self
            .builder
            .build_pointer_cast(
                method_ptr,
                fn_type.ptr_type(AddressSpace::default()),
                "method_fn",
            )
            .map_err(|e| e.to_string())?;

        let mut all_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            vec![receiver.into()];
        all_args.extend_from_slice(extra_args);

        let call_result = self
            .builder
            .build_indirect_call(fn_type, fn_ptr, &all_args, "call_vtable")
            .map_err(|e| e.to_string())?;
        let value = call_result
            .try_as_basic_value()
            .left()
            .ok_or_else(|| format!("El metodo {} no devolvio un valor", method_name))?;

        match info.ret {
            ValueKind::Number => Ok(CodegenValue::Number(value.into_float_value())),
            ValueKind::Bool => Ok(CodegenValue::Bool(value.into_int_value())),
            ValueKind::String => Ok(CodegenValue::String(value.into_pointer_value())),
            ValueKind::Object => Ok(CodegenValue::Object(value.into_pointer_value())),
            ValueKind::Vector => Ok(CodegenValue::Vector(value.into_pointer_value())),
            ValueKind::Closure => Ok(CodegenValue::Closure(value.into_pointer_value())),
        }
    }

    /// Dispatch de protocolo: dado un objeto cuyo tipo estatico es un protocolo,
    /// genera un switch sobre el type_id en runtime para llamar al metodo correcto
    /// del tipo concreto subyacente.
    pub(super) fn emit_protocol_dispatch(
        &mut self,
        receiver: PointerValue<'ctx>,
        protocol_name: &str,
        method_name: &str,
        extra_args: &[inkwell::values::BasicMetadataValueEnum<'ctx>],
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        use inkwell::{AddressSpace, IntPredicate};

        let conforming = self.types_conforming_to_protocol(protocol_name, analysis);
        if conforming.is_empty() {
            return Err(format!(
                "Ningun tipo concreto implementa el protocolo {} con el metodo {}",
                protocol_name, method_name
            ));
        }

        let ret_kind = self.effective_protocol_method_return_kind(protocol_name, method_name, analysis)?;

        let type_id = {
            let anchor = conforming[0].clone();
            let object_struct = self.object_struct_type(&anchor)?;
            let typed_ptr = self.cast_object_ptr(&receiver, &anchor)?;
            let vtable_slot = self
                .builder
                .build_struct_gep(object_struct, typed_ptr, 0, "proto_vtable_slot")
                .map_err(|e| e.to_string())?;
            let vtable_ptr = self
                .builder
                .build_load(
                    self.context.i8_type().ptr_type(AddressSpace::default()),
                    vtable_slot,
                    "proto_vtable_ptr",
                )
                .map_err(|e| e.to_string())?
                .into_pointer_value();
            let type_id_ptr = self
                .builder
                .build_pointer_cast(
                    vtable_ptr,
                    self.context.i64_type().ptr_type(AddressSpace::default()),
                    "proto_type_id_ptr",
                )
                .map_err(|e| e.to_string())?;
            self.builder
                .build_load(self.context.i64_type(), type_id_ptr, "proto_type_id")
                .map_err(|e| e.to_string())?
                .into_int_value()
        };

        let result_ptr = self.alloca_for_kind(&ret_kind, "proto_result")?;
        let default_val = self.default_value_for_kind(ret_kind)?;
        self.store_value(result_ptr, default_val)?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| "No hay funcion activa para dispatch de protocolo".to_string())?;

        let after_block = self.context.append_basic_block(function, "proto_after");

        let mut cur_block = self.context.append_basic_block(function, "proto_dispatch");
        self.builder
            .build_unconditional_branch(cur_block)
            .map_err(|e| e.to_string())?;

        for type_name in &conforming {
            self.builder.position_at_end(cur_block);

            let expected_id = self.type_id_for(type_name)?;
            let expected_val = self.context.i64_type().const_int(expected_id, false);
            let cmp = self
                .builder
                .build_int_compare(IntPredicate::EQ, type_id, expected_val, "proto_cmp")
                .map_err(|e| e.to_string())?;

            let match_block = self
                .context
                .append_basic_block(function, &format!("proto_match_{}", type_name));
            let next_block = self
                .context
                .append_basic_block(function, &format!("proto_next_{}", type_name));

            self.builder
                .build_conditional_branch(cmp, match_block, next_block)
                .map_err(|e| e.to_string())?;

            self.builder.position_at_end(match_block);

            let owner = self.object_method_owner(type_name, method_name, analysis)?;
            let symbol = self.method_symbol_name(&owner, method_name);
            let info = self
                .get_function(&symbol)
                .cloned()
                .ok_or_else(|| format!("Metodo no encontrado: {}", symbol))?;

            let result =
                self.emit_vtable_call(receiver, type_name, method_name, &info, extra_args)?;
            let coerced = self.coerce_value_to_kind(result, ret_kind)?;
            self.store_value(result_ptr, coerced)?;
            self.builder
                .build_unconditional_branch(after_block)
                .map_err(|e| e.to_string())?;

            cur_block = next_block;
        }

        self.builder.position_at_end(cur_block);
        let panic_fn = self.get_panic_function();
        let msg_name = self.fresh_tmp("proto_panic_msg");
        let msg = self
            .builder
            .build_global_string_ptr(
                &format!(
                    "Runtime error: tipo desconocido en dispatch de protocolo {}",
                    protocol_name
                ),
                &msg_name,
            )
            .map_err(|e| e.to_string())?;
        self.builder
            .build_call(panic_fn, &[msg.as_pointer_value().into()], "proto_panic")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unreachable()
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(after_block);
        self.load_value(&ret_kind, result_ptr, "proto_result_load")
    }

    /// Coerciona un valor al ValueKind objetivo. Soporta reinterpretaciones entre
    /// tipos puntero (String/Object/Vector/Closure). Falla si se intenta coaccionar
    /// un primitivo a un puntero o viceversa.
    pub(super) fn coerce_value_to_kind(
        &mut self,
        value: CodegenValue<'ctx>,
        target: ValueKind,
    ) -> Result<CodegenValue<'ctx>, String> {
        use inkwell::AddressSpace;

        if value.kind() == target {
            return Ok(value);
        }

        let i8_ptr = self.context.i8_type().ptr_type(AddressSpace::default());

        match (value, target) {
            (CodegenValue::String(p), ValueKind::Object) => Ok(CodegenValue::Object(p)),
            (CodegenValue::Object(p), ValueKind::String) => Ok(CodegenValue::String(p)),
            (CodegenValue::Vector(p), ValueKind::Object) => {
                let cast = self
                    .builder
                    .build_pointer_cast(p, i8_ptr, "vec_to_obj")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Object(cast))
            }
            (CodegenValue::Closure(p), ValueKind::Object) => {
                let cast = self
                    .builder
                    .build_pointer_cast(p, i8_ptr, "closure_to_obj")
                    .map_err(|e| e.to_string())?;
                Ok(CodegenValue::Object(cast))
            }
            (actual, expected) => Err(format!(
                "No se puede coaccionar {:?} a {:?} en dispatch de protocolo",
                actual.kind(),
                expected
            )),
        }
    }
}
