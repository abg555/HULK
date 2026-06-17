use std::collections::{HashMap, HashSet};

use inkwell::types::{BasicMetadataTypeEnum, BasicType};
use inkwell::AddressSpace;

use crate::ast::{Expr, KindExpr, LambdaExpr, Pattern};
use crate::semantic::types::SemanticType;
use crate::semantic::SemanticAnalysis;

use super::super::{CodeGenerator, CodegenValue, ValueKind, VarInfo};

pub(super) fn collect_free_vars(expr: &Expr, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    match &expr.kind {
        KindExpr::Literal(_) => {}
        KindExpr::Variable(var) => {
            if !bound.contains(&var.name) {
                free.insert(var.name.clone());
            }
        }
        KindExpr::Binary(bin) => {
            collect_free_vars(&bin.left, bound, free);
            collect_free_vars(&bin.right, bound, free);
        }
        KindExpr::Unary(un) => collect_free_vars(&un.right, bound, free),
        KindExpr::Call(call) => {
            collect_free_vars(&call.callee, bound, free);
            for arg in &call.arguments {
                collect_free_vars(arg, bound, free);
            }
        }
        KindExpr::BaseCall(base) => {
            for arg in &base.arguments {
                collect_free_vars(arg, bound, free);
            }
        }
        KindExpr::MacroCall(macro_call) => {
            for arg in &macro_call.arguments {
                collect_free_vars(&arg.value, bound, free);
            }
            if let Some(action) = &macro_call.action {
                collect_free_vars(action, bound, free);
            }
        }
        KindExpr::Let(let_expr) => {
            let mut added = Vec::new();
            for binding in &let_expr.bindings {
                collect_free_vars(&binding.initializer, bound, free);
                if bound.insert(binding.name.clone()) {
                    added.push(binding.name.clone());
                }
            }
            collect_free_vars(&let_expr.body, bound, free);
            for name in added {
                bound.remove(&name);
            }
        }
        KindExpr::Block(block) => {
            for e in &block.expressions {
                collect_free_vars(e, bound, free);
            }
        }
        KindExpr::If(if_expr) => {
            collect_free_vars(&if_expr.condition, bound, free);
            collect_free_vars(&if_expr.then_branch, bound, free);
            for (cond, body) in &if_expr.elif_branches {
                collect_free_vars(cond, bound, free);
                collect_free_vars(body, bound, free);
            }
            collect_free_vars(&if_expr.else_branch, bound, free);
        }
        KindExpr::While(while_expr) => {
            collect_free_vars(&while_expr.condition, bound, free);
            collect_free_vars(&while_expr.body, bound, free);
        }
        KindExpr::For(for_expr) => {
            collect_free_vars(&for_expr.iterable, bound, free);
            let added = bound.insert(for_expr.variable.clone());
            collect_free_vars(&for_expr.body, bound, free);
            if added {
                bound.remove(&for_expr.variable);
            }
        }
        KindExpr::Assign(assign) => {
            collect_free_vars(&assign.target, bound, free);
            collect_free_vars(&assign.value, bound, free);
        }
        KindExpr::MemberAccess(member) => collect_free_vars(&member.object, bound, free),
        KindExpr::Index(index) => {
            collect_free_vars(&index.object, bound, free);
            collect_free_vars(&index.index, bound, free);
        }
        KindExpr::Array(array) => {
            for e in &array.elements {
                collect_free_vars(e, bound, free);
            }
        }
        KindExpr::ArrayComprehension(comp) => {
            collect_free_vars(&comp.iterable, bound, free);
            let added = bound.insert(comp.variable.clone());
            collect_free_vars(&comp.element, bound, free);
            if added {
                bound.remove(&comp.variable);
            }
        }
        KindExpr::Lambda(lambda) => {
            let mut added = Vec::new();
            for param in &lambda.params {
                if bound.insert(param.name.clone()) {
                    added.push(param.name.clone());
                }
            }
            collect_free_vars(&lambda.body, bound, free);
            for name in added {
                bound.remove(&name);
            }
        }
        KindExpr::New(new_expr) => {
            for arg in &new_expr.arguments {
                collect_free_vars(arg, bound, free);
            }
        }
        KindExpr::Is(is_expr) => collect_free_vars(&is_expr.expression, bound, free),
        KindExpr::As(as_expr) => collect_free_vars(&as_expr.expression, bound, free),
        KindExpr::Match(match_expr) => {
            collect_free_vars(&match_expr.expression, bound, free);
            for case in &match_expr.cases {
                let mut names = Vec::new();
                collect_pattern_bound_names(&case.pattern, &mut names);
                let mut added = Vec::new();
                for name in names {
                    if bound.insert(name.clone()) {
                        added.push(name);
                    }
                }
                collect_free_vars(&case.body, bound, free);
                for name in added {
                    bound.remove(&name);
                }
            }
        }
    }
}

fn collect_pattern_bound_names(pattern: &Pattern, names: &mut Vec<String>) {
    match pattern {
        Pattern::Identifier { name, .. } => names.push(name.clone()),
        Pattern::Binary { left, right, .. } => {
            collect_pattern_bound_names(left, names);
            collect_pattern_bound_names(right, names);
        }
        Pattern::Unary { operand, .. } => collect_pattern_bound_names(operand, names),
        Pattern::Literal(_) | Pattern::Default => {}
    }
}

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_lambda(
        &mut self,
        lambda_node: &Expr,
        lambda: &LambdaExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let signature = analysis
            .inferred_types
            .get(&lambda_node.id)
            .ok_or_else(|| "No se encontro el tipo inferido de la funcion lambda".to_string())?
            .clone();
        let SemanticType::Function(param_types, ret_type) = signature else {
            return Err("Se esperaba un tipo funcion para la lambda".to_string());
        };

        let param_kinds: Vec<ValueKind> = param_types
            .iter()
            .map(|t| self.value_kind_from_semantic(t))
            .collect::<Result<_, _>>()?;
        let ret_kind = self.value_kind_from_semantic(&ret_type)?;

        let mut bound = HashSet::new();
        for param in &lambda.params {
            bound.insert(param.name.clone());
        }
        let mut free = HashSet::new();
        collect_free_vars(&lambda.body, &mut bound, &mut free);

        let mut captures: Vec<(String, VarInfo<'ctx>)> = free
            .into_iter()
            .filter_map(|name| self.lookup_var(&name).map(|info| (name, info)))
            .collect();
        captures.sort_by(|a, b| a.0.cmp(&b.0));

        let capture_basic_types = captures
            .iter()
            .map(|(_, info)| self.basic_type_for_kind(&info.kind))
            .collect::<Result<Vec<_>, _>>()?;
        let env_struct = self.context.struct_type(&capture_basic_types, false);

        let malloc_fn = self.get_malloc_function();
        let i8_ptr_type = self.context.i8_type().ptr_type(AddressSpace::default());

        let env_size = match env_struct.size_of() {
            Some(size) => self
                .builder
                .build_int_cast(size, self.context.i64_type(), "env_size")
                .map_err(|e| e.to_string())?,
            None => self.context.i64_type().const_int(0, false),
        };
        let env_raw_ptr = self
            .builder
            .build_call(malloc_fn, &[env_size.into()], "env_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();

        let env_typed_ptr = self
            .builder
            .build_pointer_cast(
                env_raw_ptr,
                env_struct.ptr_type(AddressSpace::default()),
                "env_typed",
            )
            .map_err(|e| e.to_string())?;

        for (idx, (name, info)) in captures.iter().enumerate() {
            let value = self.load_value(&info.kind, info.ptr, &format!("capture_{}", name))?;
            let field_ptr = self
                .builder
                .build_struct_gep(env_struct, env_typed_ptr, idx as u32, &format!("capture_slot_{}", name))
                .map_err(|e| e.to_string())?;
            self.store_value(field_ptr, value)?;
        }

        let mut param_basic_types: Vec<BasicMetadataTypeEnum> = vec![i8_ptr_type.into()];
        for kind in &param_kinds {
            param_basic_types.push(self.basic_type_for_kind(kind)?.into());
        }
        let ret_basic_type = self.basic_type_for_kind(&ret_kind)?;
        let fn_type = ret_basic_type.fn_type(&param_basic_types, false);

        let fn_name = self.fresh_tmp("lambda");
        let function = self.module.add_function(&fn_name, fn_type, None);

        let saved_block = self.builder.get_insert_block();
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![HashMap::new()]);

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);

        let env_arg = function
            .get_nth_param(0)
            .ok_or_else(|| "Falta el entorno de la funcion lambda".to_string())?;
        env_arg.set_name("env");
        let env_arg_typed = self
            .builder
            .build_pointer_cast(
                env_arg.into_pointer_value(),
                env_struct.ptr_type(AddressSpace::default()),
                "env_arg_typed",
            )
            .map_err(|e| e.to_string())?;

        for (idx, (name, info)) in captures.iter().enumerate() {
            let field_ptr = self
                .builder
                .build_struct_gep(env_struct, env_arg_typed, idx as u32, &format!("capture_load_slot_{}", name))
                .map_err(|e| e.to_string())?;
            let ptr = self.alloca_for_kind(&info.kind, name)?;
            let value = self.load_value(&info.kind, field_ptr, &format!("capture_load_{}", name))?;
            self.store_value(ptr, value)?;
            self.insert_var(name.clone(), VarInfo { ptr, kind: info.kind });
        }

        for (idx, param) in lambda.params.iter().enumerate() {
            let arg = function
                .get_nth_param((idx + 1) as u32)
                .ok_or_else(|| "Parametro faltante en lambda".to_string())?;
            arg.set_name(&param.name);

            let kind = param_kinds[idx];
            let ptr = self.alloca_for_kind(&kind, &param.name)?;

            match kind {
                ValueKind::Number => {
                    self.builder
                        .build_store(ptr, arg.into_float_value())
                        .map_err(|e| e.to_string())?;
                }
                ValueKind::Bool => {
                    self.builder
                        .build_store(ptr, arg.into_int_value())
                        .map_err(|e| e.to_string())?;
                }
                ValueKind::String | ValueKind::Object | ValueKind::Vector | ValueKind::Closure => {
                    self.builder
                        .build_store(ptr, arg.into_pointer_value())
                        .map_err(|e| e.to_string())?;
                }
            }

            self.insert_var(param.name.clone(), VarInfo { ptr, kind });
        }

        let body_value = self.lower_expr(&lambda.body, analysis)?;
        match ret_kind {
            ValueKind::Number => {
                let value = body_value.into_number()?;
                self.builder.build_return(Some(&value)).map_err(|e| e.to_string())?;
            }
            ValueKind::Bool => {
                let value = body_value.into_bool()?;
                self.builder.build_return(Some(&value)).map_err(|e| e.to_string())?;
            }
            ValueKind::String => {
                let value = body_value.into_string()?;
                self.builder.build_return(Some(&value)).map_err(|e| e.to_string())?;
            }
            ValueKind::Object => {
                let value = body_value.into_object()?;
                self.builder.build_return(Some(&value)).map_err(|e| e.to_string())?;
            }
            ValueKind::Vector => {
                let value = body_value.into_vector()?;
                self.builder.build_return(Some(&value)).map_err(|e| e.to_string())?;
            }
            ValueKind::Closure => {
                let value = body_value.into_closure()?;
                self.builder.build_return(Some(&value)).map_err(|e| e.to_string())?;
            }
        }

        self.scopes = saved_scopes;
        if let Some(block) = saved_block {
            self.builder.position_at_end(block);
        }

        let closure_size = match self.closure_struct.size_of() {
            Some(size) => self
                .builder
                .build_int_cast(size, self.context.i64_type(), "closure_size")
                .map_err(|e| e.to_string())?,
            None => self.context.i64_type().const_int(0, false),
        };
        let closure_raw_ptr = self
            .builder
            .build_call(malloc_fn, &[closure_size.into()], "closure_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();
        let closure_typed_ptr = self
            .builder
            .build_pointer_cast(
                closure_raw_ptr,
                self.closure_struct.ptr_type(AddressSpace::default()),
                "closure_typed",
            )
            .map_err(|e| e.to_string())?;

        let fn_ptr_slot = self
            .builder
            .build_struct_gep(self.closure_struct, closure_typed_ptr, 0, "closure_fn_slot")
            .map_err(|e| e.to_string())?;
        let fn_ptr_value = function.as_global_value().as_pointer_value().const_cast(i8_ptr_type);
        self.builder
            .build_store(fn_ptr_slot, fn_ptr_value)
            .map_err(|e| e.to_string())?;

        let env_ptr_slot = self
            .builder
            .build_struct_gep(self.closure_struct, closure_typed_ptr, 1, "closure_env_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(env_ptr_slot, env_raw_ptr)
            .map_err(|e| e.to_string())?;

        Ok(CodegenValue::Closure(closure_typed_ptr))
    }

    /// Envuelve una funcion de nivel superior (declarada con `function` o
    /// como metodo) en un valor closure, para que pueda usarse donde se
    /// espera un valor funcion (p.ej. un argumento de tipo protocolo functor).
    /// La funcion no captura nada, asi que el env es null; como su firma real
    /// no tiene el parametro `env` implicito de las lambdas, se genera (una
    /// sola vez, cacheada por nombre) una funcion *thunk* que lo ignora y
    /// reenvia la llamada a la funcion real.
    pub(super) fn function_value_as_closure(&mut self, fn_name: &str) -> Result<CodegenValue<'ctx>, String> {
        let thunk_function = self.get_or_create_function_thunk(fn_name)?;
        let i8_ptr_type = self.context.i8_type().ptr_type(AddressSpace::default());

        let malloc_fn = self.get_malloc_function();
        let closure_size = match self.closure_struct.size_of() {
            Some(size) => self
                .builder
                .build_int_cast(size, self.context.i64_type(), "closure_size")
                .map_err(|e| e.to_string())?,
            None => self.context.i64_type().const_int(0, false),
        };
        let closure_raw_ptr = self
            .builder
            .build_call(malloc_fn, &[closure_size.into()], "closure_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();
        let closure_typed_ptr = self
            .builder
            .build_pointer_cast(
                closure_raw_ptr,
                self.closure_struct.ptr_type(AddressSpace::default()),
                "closure_typed",
            )
            .map_err(|e| e.to_string())?;

        let fn_ptr_slot = self
            .builder
            .build_struct_gep(self.closure_struct, closure_typed_ptr, 0, "closure_fn_slot")
            .map_err(|e| e.to_string())?;
        let fn_ptr_value = thunk_function.as_global_value().as_pointer_value().const_cast(i8_ptr_type);
        self.builder
            .build_store(fn_ptr_slot, fn_ptr_value)
            .map_err(|e| e.to_string())?;

        let env_ptr_slot = self
            .builder
            .build_struct_gep(self.closure_struct, closure_typed_ptr, 1, "closure_env_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(env_ptr_slot, i8_ptr_type.const_null())
            .map_err(|e| e.to_string())?;

        Ok(CodegenValue::Closure(closure_typed_ptr))
    }

    fn get_or_create_function_thunk(
        &mut self,
        fn_name: &str,
    ) -> Result<inkwell::values::FunctionValue<'ctx>, String> {
        if let Some(thunk) = self.thunk_functions.get(fn_name) {
            return Ok(*thunk);
        }

        let info = self
            .functions
            .get(fn_name)
            .cloned()
            .ok_or_else(|| format!("Funcion no encontrada: {}", fn_name))?;

        let i8_ptr_type = self.context.i8_type().ptr_type(AddressSpace::default());
        let mut thunk_param_types: Vec<BasicMetadataTypeEnum> = vec![i8_ptr_type.into()];
        for kind in &info.params {
            thunk_param_types.push(self.basic_type_for_kind(kind)?.into());
        }
        let ret_basic_type = self.basic_type_for_kind(&info.ret)?;
        let thunk_fn_type = ret_basic_type.fn_type(&thunk_param_types, false);

        let thunk_name = self.fresh_tmp(&format!("{}_thunk", fn_name));
        let thunk_function = self.module.add_function(&thunk_name, thunk_fn_type, None);

        let saved_block = self.builder.get_insert_block();
        let entry = self.context.append_basic_block(thunk_function, "entry");
        self.builder.position_at_end(entry);

        let mut call_args: Vec<inkwell::values::BasicMetadataValueEnum> = Vec::new();
        for idx in 0..info.params.len() {
            let arg = thunk_function
                .get_nth_param((idx + 1) as u32)
                .ok_or_else(|| "Parametro faltante en thunk".to_string())?;
            call_args.push(arg.into());
        }
        let call_site = self
            .builder
            .build_call(info.function, &call_args, "thunk_call")
            .map_err(|e| e.to_string())?;
        match call_site.try_as_basic_value().left() {
            Some(value) => {
                self.builder.build_return(Some(&value)).map_err(|e| e.to_string())?;
            }
            None => {
                self.builder.build_return(None).map_err(|e| e.to_string())?;
            }
        }

        if let Some(block) = saved_block {
            self.builder.position_at_end(block);
        }

        self.thunk_functions.insert(fn_name.to_string(), thunk_function);
        Ok(thunk_function)
    }
}
