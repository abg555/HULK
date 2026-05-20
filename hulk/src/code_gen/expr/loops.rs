use crate::ast::{ForExpr, KindExpr, WhileExpr};
use crate::semantic::SemanticAnalysis;

use super::super::{CodegenValue, CodeGenerator, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_while(
        &mut self,
        while_expr: &WhileExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let result_kind = self.value_kind_for_expr(&while_expr.body, analysis)?;

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
        self.builder
            .build_unconditional_branch(cond_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(after_block);
        let default_value = self.default_value_for_kind(result_kind)?;

        let phi = match result_kind {
            ValueKind::Number => {
                let phi = self
                    .builder
                    .build_phi(self.f64_type, "whiletmp")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&body_value.into_number()?, body_block),
                    (&default_value.into_number()?, current_block),
                ]);
                CodegenValue::Number(phi.as_basic_value().into_float_value())
            }
            ValueKind::Bool => {
                let phi = self
                    .builder
                    .build_phi(self.bool_type, "whiletmp_bool")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&body_value.into_bool()?, body_block),
                    (&default_value.into_bool()?, current_block),
                ]);
                CodegenValue::Bool(phi.as_basic_value().into_int_value())
            }
            ValueKind::String => {
                let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let phi = self
                    .builder
                    .build_phi(i8_ptr_type, "whiletmp_str")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&body_value.into_string()?, body_block),
                    (&default_value.into_string()?, current_block),
                ]);
                CodegenValue::String(phi.as_basic_value().into_pointer_value())
            }
            ValueKind::Object => {
                let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let phi = self
                    .builder
                    .build_phi(i8_ptr_type, "whiletmp_obj")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&body_value.into_object()?, body_block),
                    (&default_value.into_object()?, current_block),
                ]);
                CodegenValue::Object(phi.as_basic_value().into_pointer_value())
            }
        };

        Ok(phi)
    }

    pub(super) fn lower_for(
        &mut self,
        for_expr: &ForExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let KindExpr::Call(call) = &for_expr.iterable.kind else {
            return Err("Solo se soporta for sobre range(...)".to_string());
        };

        let KindExpr::Variable(callee) = &call.callee.kind else {
            return Err("Solo se soporta for sobre range(...)".to_string());
        };

        if callee.name != "range" || call.arguments.len() != 2 {
            return Err("Solo se soporta for sobre range(start, end)".to_string());
        }

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

        let current_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| "No hay bloque de insercion activo".to_string())?;
        let function = current_block
            .get_parent()
            .ok_or_else(|| "No hay funcion activa".to_string())?;

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
        let result_kind = self.value_kind_for_expr(&for_expr.body, analysis)?;
        let default_value = self.default_value_for_kind(result_kind)?;

        let phi = match result_kind {
            ValueKind::Number => {
                let phi = self
                    .builder
                    .build_phi(self.f64_type, "fortmp")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&body_value.into_number()?, body_block),
                    (&default_value.into_number()?, current_block),
                ]);
                CodegenValue::Number(phi.as_basic_value().into_float_value())
            }
            ValueKind::Bool => {
                let phi = self
                    .builder
                    .build_phi(self.bool_type, "fortmp_bool")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&body_value.into_bool()?, body_block),
                    (&default_value.into_bool()?, current_block),
                ]);
                CodegenValue::Bool(phi.as_basic_value().into_int_value())
            }
            ValueKind::String => {
                let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let phi = self
                    .builder
                    .build_phi(i8_ptr_type, "fortmp_str")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&body_value.into_string()?, body_block),
                    (&default_value.into_string()?, current_block),
                ]);
                CodegenValue::String(phi.as_basic_value().into_pointer_value())
            }
            ValueKind::Object => {
                let i8_ptr_type = self.context.i8_type().ptr_type(inkwell::AddressSpace::default());
                let phi = self
                    .builder
                    .build_phi(i8_ptr_type, "fortmp_obj")
                    .map_err(|e| e.to_string())?;
                phi.add_incoming(&[
                    (&body_value.into_object()?, body_block),
                    (&default_value.into_object()?, current_block),
                ]);
                CodegenValue::Object(phi.as_basic_value().into_pointer_value())
            }
        };

        Ok(phi)
    }
}
