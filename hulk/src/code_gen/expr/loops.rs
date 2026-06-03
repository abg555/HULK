use crate::ast::{ForExpr, KindExpr, WhileExpr};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::super::{CodegenValue, CodeGenerator, ValueKind, VarInfo};

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
        let iterable_value = self.lower_expr(&for_expr.iterable, analysis)?;
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
}
