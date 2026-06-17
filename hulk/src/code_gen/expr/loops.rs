use crate::ast::{ForExpr, KindExpr, WhileExpr};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::super::{CodegenValue, CodeGenerator, FunctionInfo, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_while(
        &mut self,
        while_expr: &WhileExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let result_kind = self.value_kind_for_expr(&while_expr.body, analysis)?;
        let result_ptr = self.alloca_for_kind(&result_kind, "while_result")?;
        let default_value = self.default_value_for_kind(result_kind)?;
        self.store_value(result_ptr, default_value)?;

        let current_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| "No hay bloque de insercion activo".to_string())?;
        let function = current_block
            .get_parent()
            .ok_or_else(|| "No hay funcion activa".to_string())?;

        let cond_block = self.context.append_basic_block(function, "while_cond");
        let body_block = self.context.append_basic_block(function, "while_body");
        let after_block = self.context.append_basic_block(function, "while_after");

        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cond_block);
        let cond_value = self.lower_expr(&while_expr.condition, analysis)?.into_bool()?;
        self.builder
            .build_conditional_branch(cond_value, body_block, after_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(body_block);
        let body_value = self.lower_expr(&while_expr.body, analysis)?;
        self.store_value(result_ptr, body_value)?;
        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(after_block);
        self.load_value(&result_kind, result_ptr, "while_result")
    }

    pub(super) fn lower_for(
        &mut self,
        for_expr: &ForExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let current_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| "No hay bloque de insercion activo".to_string())?;
        let function = current_block
            .get_parent()
            .ok_or_else(|| "No hay funcion activa".to_string())?;

        let result_kind = self.value_kind_for_expr(&for_expr.body, analysis)?;
        let result_ptr = self.alloca_for_kind(&result_kind, "for_result")?;
        let default_value = self.default_value_for_kind(result_kind)?;
        self.store_value(result_ptr, default_value)?;

        if let KindExpr::Call(call) = &for_expr.iterable.kind
            && let KindExpr::Variable(callee) = &call.callee.kind
            && callee.name == "range"
            && call.arguments.len() == 2
        {
            return self.lower_for_range(for_expr, call, result_kind, result_ptr, function, analysis);
        }

        if let Some(SemanticType::Custom(type_name)) = analysis.inferred_types.get(&for_expr.iterable.id) {
            return self.lower_for_object(for_expr, result_kind, result_ptr, function, analysis, type_name.clone());
        }

        self.lower_for_vector(for_expr, result_kind, result_ptr, function, analysis)
    }

    fn lower_for_range(
        &mut self,
        for_expr: &ForExpr,
        call: &crate::ast::CallExpr,
        result_kind: ValueKind,
        result_ptr: inkwell::values::PointerValue<'ctx>,
        function: inkwell::values::FunctionValue<'ctx>,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let start = self.lower_expr(&call.arguments[0], analysis)?.into_number()?;
        let end = self.lower_expr(&call.arguments[1], analysis)?.into_number()?;

        let counter_name = self.fresh_tmp("for_i");
        let counter_ptr = self
            .builder
            .build_alloca(self.f64_type, &counter_name)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(counter_ptr, start)
            .map_err(|e| e.to_string())?;

        let cond_block = self.context.append_basic_block(function, "for_cond");
        let body_block = self.context.append_basic_block(function, "for_body");
        let after_block = self.context.append_basic_block(function, "for_after");

        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cond_block);
        let counter_value = self
            .builder
            .build_load(self.f64_type, counter_ptr, "for_i_load")
            .map_err(|e| e.to_string())?
            .into_float_value();
        let cond_value = self
            .builder
            .build_float_compare(
                inkwell::FloatPredicate::OLT,
                counter_value,
                end,
                "for_cmp",
            )
            .map_err(|e| e.to_string())?;
        self.builder
            .build_conditional_branch(cond_value, body_block, after_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(body_block);
        self.enter_scope();
        let loop_var_ptr = self
            .builder
            .build_alloca(self.f64_type, &for_expr.variable)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(loop_var_ptr, counter_value)
            .map_err(|e| e.to_string())?;
        self.insert_var(
            for_expr.variable.clone(),
            VarInfo {
                ptr: loop_var_ptr,
                kind: ValueKind::Number,
            },
        );

        let body_value = self.lower_expr(&for_expr.body, analysis)?;
        self.store_value(result_ptr, body_value)?;
        self.exit_scope();

        let next_value = self
            .builder
            .build_float_add(
                counter_value,
                self.f64_type.const_float(1.0),
                "for_next",
            )
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(counter_ptr, next_value)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(after_block);
        self.load_value(&result_kind, result_ptr, "for_result")
    }

    fn lower_for_vector(
        &mut self,
        for_expr: &ForExpr,
        result_kind: ValueKind,
        result_ptr: inkwell::values::PointerValue<'ctx>,
        function: inkwell::values::FunctionValue<'ctx>,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        // Evaluate iterable once.
        let iterable_value = self.lower_expr(&for_expr.iterable, analysis)?;

        // If at runtime we got an object, prefer protocol-based iteration
        // (Iterable) so we don't rely on brittle AST heuristics to recover
        // a concrete type name for variables passed as Vector parameters.
        if let CodegenValue::Object(obj_ptr) = iterable_value {
            let iterable_slot = self.alloca_for_kind(&ValueKind::Object, "for_obj_iter")?;
            self.store_value(iterable_slot, CodegenValue::Object(obj_ptr))?;

            // If the static inferred type names a Custom type, and it's not a
            // protocol, we can try concrete dispatch; otherwise fall back to
            // protocol dispatch on Iterable.
            if let Some(SemanticType::Custom(type_name)) =
                analysis.inferred_types.get(&for_expr.iterable.id)
            {
                if self.is_protocol_name(type_name, analysis) {
                    return self.lower_for_object_protocol(
                        for_expr,
                        result_kind,
                        result_ptr,
                        function,
                        analysis,
                        type_name,
                        iterable_slot,
                    );
                } else {
                    return self.lower_for_object(
                        for_expr,
                        result_kind,
                        result_ptr,
                        function,
                        analysis,
                        type_name.clone(),
                    );
                }
            }

            return self.lower_for_object_protocol(
                for_expr,
                result_kind,
                result_ptr,
                function,
                analysis,
                "Iterable",
                iterable_slot,
            );
        }

        let vec_ptr = iterable_value.into_vector().map_err(|_| {
            "Codegen de for solo soporta iterables vectoriales y range(start, end)".to_string()
        })?;

        let Some(SemanticType::Vector(inner)) = analysis.inferred_types.get(&for_expr.iterable.id)
        else {
            return Err(
                "No se encontro tipo inferido del iterable vectorial en for".to_string(),
            );
        };
        let elem_sem = *inner.clone();
        let elem_kind = self.value_kind_from_semantic(&elem_sem)?;

        // Evaluate iterable exactly once and keep the same vector reference
        // across next/current calls inside the loop.
        let iterable_slot = self.alloca_for_kind(&ValueKind::Vector, "for_iterable")?;
        self.store_value(iterable_slot, CodegenValue::Vector(vec_ptr))?;

        let cond_block = self.context.append_basic_block(function, "for_cond");
        let body_block = self.context.append_basic_block(function, "for_body");
        let after_block = self.context.append_basic_block(function, "for_after");

        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cond_block);
        let iterable_in_cond = self
            .load_value(&ValueKind::Vector, iterable_slot, "for_iterable_cond")?
            .into_vector()?;
        let cond_value = self.vector_next(iterable_in_cond)?;
        self.builder
            .build_conditional_branch(cond_value, body_block, after_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(body_block);
        let iterable_in_body = self
            .load_value(&ValueKind::Vector, iterable_slot, "for_iterable_body")?
            .into_vector()?;
        let elem_value = self.vector_current(iterable_in_body, &elem_sem)?;

        self.enter_scope();
        let loop_var_ptr = self.alloca_for_kind(&elem_kind, &for_expr.variable)?;
        self.store_value(loop_var_ptr, elem_value)?;
        self.insert_var(
            for_expr.variable.clone(),
            VarInfo {
                ptr: loop_var_ptr,
                kind: elem_kind,
            },
        );

        let body_value = self.lower_expr(&for_expr.body, analysis)?;
        self.store_value(result_ptr, body_value)?;
        self.exit_scope();
        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(after_block);
        self.load_value(&result_kind, result_ptr, "for_result")
    }

    fn lower_for_object(
        &mut self,
        for_expr: &ForExpr,
        result_kind: ValueKind,
        result_ptr: inkwell::values::PointerValue<'ctx>,
        function: inkwell::values::FunctionValue<'ctx>,
        analysis: &SemanticAnalysis,
        type_name: String,
    ) -> Result<CodegenValue<'ctx>, String> {
        let iterable_value = self.lower_expr(&for_expr.iterable, analysis)?.into_object()?;
        let iterable_slot = self.alloca_for_kind(&ValueKind::Object, "for_obj_iter")?;
        self.store_value(iterable_slot, CodegenValue::Object(iterable_value))?;

        if self.is_protocol_name(&type_name, analysis) {
            return self.lower_for_object_protocol(
                for_expr,
                result_kind,
                result_ptr,
                function,
                analysis,
                &type_name,
                iterable_slot,
            );
        }

        let next_owner = self.object_method_owner(&type_name, "next", analysis)?;
        let next_symbol = self.method_symbol_name(&next_owner, "next");
        let next_info = self
            .get_function(&next_symbol)
            .cloned()
            .ok_or_else(|| format!("Metodo next no encontrado en {}", type_name))?;

        let current_owner = self.object_method_owner(&type_name, "current", analysis)?;
        let current_symbol = self.method_symbol_name(&current_owner, "current");
        let current_info = self
            .get_function(&current_symbol)
            .cloned()
            .ok_or_else(|| format!("Metodo current no encontrado en {}", type_name))?;
        let elem_kind = current_info.ret;

        let cond_block = self.context.append_basic_block(function, "for_cond");
        let body_block = self.context.append_basic_block(function, "for_body");
        let after_block = self.context.append_basic_block(function, "for_after");

        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cond_block);
        let receiver = self
            .load_value(&ValueKind::Object, iterable_slot, "for_obj_recv")?
            .into_object()?;
        let cond_value = self
            .emit_vtable_call_no_args(receiver, &type_name, "next", &next_info)?
            .into_bool()?;
        self.builder
            .build_conditional_branch(cond_value, body_block, after_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(body_block);
        self.enter_scope();
        let receiver = self
            .load_value(&ValueKind::Object, iterable_slot, "for_obj_recv2")?
            .into_object()?;
        let elem_value =
            self.emit_vtable_call_no_args(receiver, &type_name, "current", &current_info)?;
        let loop_var_ptr = self.alloca_for_kind(&elem_kind, &for_expr.variable)?;
        self.store_value(loop_var_ptr, elem_value)?;
        self.insert_var(
            for_expr.variable.clone(),
            VarInfo {
                ptr: loop_var_ptr,
                kind: elem_kind,
            },
        );

        let body_value = self.lower_expr(&for_expr.body, analysis)?;
        self.store_value(result_ptr, body_value)?;
        self.exit_scope();
        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(after_block);
        self.load_value(&result_kind, result_ptr, "for_result")
    }

    fn lower_for_object_protocol(
        &mut self,
        for_expr: &ForExpr,
        result_kind: ValueKind,
        result_ptr: inkwell::values::PointerValue<'ctx>,
        function: inkwell::values::FunctionValue<'ctx>,
        analysis: &SemanticAnalysis,
        protocol_name: &str,
        iterable_slot: inkwell::values::PointerValue<'ctx>,
    ) -> Result<CodegenValue<'ctx>, String> {
        let elem_kind = self
            .effective_protocol_method_return_kind(protocol_name, "current", analysis)?;

        let cond_block = self.context.append_basic_block(function, "for_cond");
        let body_block = self.context.append_basic_block(function, "for_body");
        let after_block = self.context.append_basic_block(function, "for_after");

        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cond_block);
        let receiver = self
            .load_value(&ValueKind::Object, iterable_slot, "for_proto_recv")?
            .into_object()?;
        let cond_value = self
            .emit_protocol_dispatch(receiver, protocol_name, "next", &[], analysis)?
            .into_bool()?;
        self.builder
            .build_conditional_branch(cond_value, body_block, after_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(body_block);
        self.enter_scope();
        let receiver = self
            .load_value(&ValueKind::Object, iterable_slot, "for_proto_recv2")?
            .into_object()?;
        let elem_value =
            self.emit_protocol_dispatch(receiver, protocol_name, "current", &[], analysis)?;
        let loop_var_ptr = self.alloca_for_kind(&elem_kind, &for_expr.variable)?;
        self.store_value(loop_var_ptr, elem_value)?;
        self.insert_var(
            for_expr.variable.clone(),
            VarInfo {
                ptr: loop_var_ptr,
                kind: elem_kind,
            },
        );

        let body_value = self.lower_expr(&for_expr.body, analysis)?;
        self.store_value(result_ptr, body_value)?;
        self.exit_scope();
        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(after_block);
        self.load_value(&result_kind, result_ptr, "for_result")
    }

    fn emit_vtable_call_no_args(
        &mut self,
        receiver: inkwell::values::PointerValue<'ctx>,
        type_name: &str,
        method_name: &str,
        info: &FunctionInfo<'ctx>,
    ) -> Result<CodegenValue<'ctx>, String> {
        self.emit_vtable_call(receiver, type_name, method_name, info, &[])
    }
}
