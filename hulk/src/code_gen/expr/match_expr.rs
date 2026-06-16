use inkwell::FloatPredicate;
use inkwell::IntPredicate;

use crate::ast::{Expr, LiteralValue, MatchExpr, Pattern};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::super::{CodeGenerator, CodegenValue, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    /// Genera IR para una expresion `match`.
    ///
    /// La estructura IR generada es:
    ///   - El escrutinio se evalúa una sola vez y se guarda en un alloca.
    ///   - Cada patron genera bloques `match_test_N` (condicion) y `match_arm_N` (cuerpo).
    ///   - Patrones catch-all (default / Identifier sin tipo) saltan directamente al arm.
    ///   - Todos los arms escriben su resultado en un alloca compartido `match_result`.
    ///   - El bloque `match_after` carga y devuelve el valor final.
    pub(super) fn lower_match(
        &mut self,
        match_node: &Expr,
        match_expr: &MatchExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let _ = match_node; // el id del nodo no se usa directamente aquí

        if match_expr.cases.is_empty() {
            return Err("Match sin casos".to_string());
        }

        // ── 1. Evaluar el escrutinio y guardarlo en un alloca ─────────────────────
        let scrut_value = self.lower_expr(&match_expr.expression, analysis)?;
        let scrut_kind = scrut_value.kind();
        let scrut_slot = self.alloca_for_kind(&scrut_kind, "match_scrut")?;
        self.store_value(scrut_slot, scrut_value)?;

        // Tipo semántico del escrutinio (para patrones de tipo / `case x: T =>`)
        let scrut_sem_type = analysis
            .inferred_types
            .get(&match_expr.expression.id)
            .cloned()
            .ok_or_else(|| "No se encontro tipo inferido del escrutinio de match".to_string())?;

        // ── 2. Alloca para el resultado ───────────────────────────────────────────
        let result_kind = self.value_kind_for_expr(&match_expr.cases[0].body, analysis)?;
        let result_slot = self.alloca_for_kind(&result_kind, "match_result")?;
        let default_val = self.default_value_for_kind(result_kind)?;
        self.store_value(result_slot, default_val)?;

        // ── 3. Bloques de control ─────────────────────────────────────────────────
        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| "No hay funcion activa para match".to_string())?;

        let after_block = self
            .context
            .append_basic_block(function, "match_after");

        // Bloque actual donde se emite la condición del siguiente caso.
        let mut dispatch_block = self
            .builder
            .get_insert_block()
            .ok_or_else(|| "No hay bloque activo al iniciar match".to_string())?;

        let last_idx = match_expr.cases.len() - 1;

        // ── 4. Iterar sobre los casos ─────────────────────────────────────────────
        for (i, case) in match_expr.cases.iter().enumerate() {
            let arm_block = self
                .context
                .append_basic_block(function, &format!("match_arm_{}", i));

            let is_catch_all = matches!(
                &case.pattern,
                Pattern::Default | Pattern::Identifier { type_restriction: None, .. }
            );

            // Emitir condición (o salto incondicional) desde el bloque de despacho
            self.builder.position_at_end(dispatch_block);

            if is_catch_all {
                self.builder
                    .build_unconditional_branch(arm_block)
                    .map_err(|e| e.to_string())?;
                // Después de un catch-all no hay más bloques de despacho que generar.
                dispatch_block = arm_block;
            } else {
                // Si es el último caso y no es catch-all, su fallthrough es after_block.
                let fallthrough = if i == last_idx {
                    after_block
                } else {
                    self.context
                        .append_basic_block(function, &format!("match_next_{}", i))
                };

                let cond = self.test_match_pattern(
                    &case.pattern,
                    scrut_slot,
                    scrut_kind,
                    &scrut_sem_type,
                    analysis,
                    i,
                )?;
                self.builder
                    .build_conditional_branch(cond, arm_block, fallthrough)
                    .map_err(|e| e.to_string())?;

                dispatch_block = fallthrough;
            }

            // ── 5. Generar el cuerpo del arm ──────────────────────────────────────
            self.builder.position_at_end(arm_block);
            self.enter_scope();
            self.bind_match_pattern_vars(
                &case.pattern,
                scrut_slot,
                scrut_kind,
                &scrut_sem_type,
            )?;
            let body_val = self.lower_expr(&case.body, analysis)?;
            self.exit_scope();

            self.store_value(result_slot, body_val)?;
            self.builder
                .build_unconditional_branch(after_block)
                .map_err(|e| e.to_string())?;
        }

        // Si dispatch_block quedó abierto (sin terminador y distinto de after_block),
        // significa que no habia un catch-all: cerrar el fallthrough (código inalcanzable
        // en un match exhaustivo, pero necesario para que el IR sea válido).
        if dispatch_block != after_block {
            self.builder.position_at_end(dispatch_block);
            if dispatch_block.get_terminator().is_none() {
                self.builder
                    .build_unconditional_branch(after_block)
                    .map_err(|e| e.to_string())?;
            }
        }

        // ── 6. Bloque de salida ───────────────────────────────────────────────────
        self.builder.position_at_end(after_block);
        self.load_value(&result_kind, result_slot, "match_val")
    }

    /// Genera la condición booleana (i1) que comprueba si el escrutinio satisface `pattern`.
    /// Solo se llama para patrones no-catch-all.
    fn test_match_pattern(
        &mut self,
        pattern: &Pattern,
        scrut_slot: inkwell::values::PointerValue<'ctx>,
        scrut_kind: ValueKind,
        scrut_sem_type: &SemanticType,
        analysis: &SemanticAnalysis,
        tag: usize,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        match pattern {
            // ── Literales numéricos ───────────────────────────────────────────────
            Pattern::Literal(LiteralValue::Number(n)) => {
                let scrut_val = self
                    .load_value(&scrut_kind, scrut_slot, "scrut_num")?
                    .into_number()?;
                let expected = self.f64_type.const_float(*n);
                self.builder
                    .build_float_compare(FloatPredicate::OEQ, scrut_val, expected, "match_num_cmp")
                    .map_err(|e| e.to_string())
            }

            // ── Literales booleanos ───────────────────────────────────────────────
            Pattern::Literal(LiteralValue::Bool(b)) => {
                let scrut_val = self
                    .load_value(&scrut_kind, scrut_slot, "scrut_bool")?
                    .into_bool()?;
                let expected = self.bool_type.const_int(u64::from(*b), false);
                self.builder
                    .build_int_compare(IntPredicate::EQ, scrut_val, expected, "match_bool_cmp")
                    .map_err(|e| e.to_string())
            }

            // ── Literales de string ───────────────────────────────────────────────
            Pattern::Literal(LiteralValue::String(s)) => {
                let scrut_val = self
                    .load_value(&scrut_kind, scrut_slot, "scrut_str")?
                    .into_string()?;
                let lit_name = self.fresh_tmp("match_str_lit");
                let expected_ptr = self
                    .builder
                    .build_global_string_ptr(s, &lit_name)
                    .map_err(|e| e.to_string())?
                    .as_pointer_value();
                let strcmp_fn = self.get_strcmp_function();
                let cmp_result = self
                    .builder
                    .build_call(
                        strcmp_fn,
                        &[scrut_val.into(), expected_ptr.into()],
                        "match_strcmp",
                    )
                    .map_err(|e| e.to_string())?
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| "strcmp no devolvio un valor".to_string())?
                    .into_int_value();
                self.builder
                    .build_int_compare(
                        IntPredicate::EQ,
                        cmp_result,
                        self.context.i32_type().const_int(0, false),
                        "match_str_eq",
                    )
                    .map_err(|e| e.to_string())
            }

            // ── Patron de tipo: `case x: Dog =>` ─────────────────────────────────
            Pattern::Identifier {
                type_restriction: Some(type_ref),
                ..
            } => {
                let target_type = SemanticType::from_type_ref(type_ref);
                let SemanticType::Custom(target_name) = target_type else {
                    return Err(
                        "Patron de tipo solo soportado para tipos custom en match".to_string()
                    );
                };
                let scrut_obj = self
                    .load_value(&scrut_kind, scrut_slot, "scrut_obj")?
                    .into_object()?;
                let SemanticType::Custom(static_name) = scrut_sem_type else {
                    return Err(
                        "El escrutinio de un patron de tipo debe ser un objeto custom".to_string()
                    );
                };
                let type_id = self.load_type_id_from_obj(scrut_obj, static_name)?;
                let ids = self.subtype_ids(&target_name, analysis)?;

                let mut result: Option<inkwell::values::IntValue<'ctx>> = None;
                for id in ids {
                    let expected_id = self.context.i64_type().const_int(id, false);
                    let cmp = self
                        .builder
                        .build_int_compare(
                            IntPredicate::EQ,
                            type_id,
                            expected_id,
                            &format!("match_type_cmp_{}", tag),
                        )
                        .map_err(|e| e.to_string())?;
                    result = Some(match result {
                        Some(acc) => self
                            .builder
                            .build_or(acc, cmp, "match_type_or")
                            .map_err(|e| e.to_string())?,
                        None => cmp,
                    });
                }

                Ok(result.unwrap_or_else(|| self.bool_type.const_int(0, false)))
            }

            // ── Patrones binarios (OR de condiciones) ─────────────────────────────
            Pattern::Binary { left, right, .. } => {
                let left_cond = self.test_match_pattern(
                    left,
                    scrut_slot,
                    scrut_kind,
                    scrut_sem_type,
                    analysis,
                    tag,
                )?;
                let right_cond = self.test_match_pattern(
                    right,
                    scrut_slot,
                    scrut_kind,
                    scrut_sem_type,
                    analysis,
                    tag,
                )?;
                self.builder
                    .build_or(left_cond, right_cond, "match_bin_or")
                    .map_err(|e| e.to_string())
            }

            // ── Patrones unarios (pass-through) ───────────────────────────────────
            Pattern::Unary { operand, .. } => self.test_match_pattern(
                operand,
                scrut_slot,
                scrut_kind,
                scrut_sem_type,
                analysis,
                tag,
            ),

            // Catch-all: este método no debería ser llamado para estos casos.
            Pattern::Default | Pattern::Identifier { type_restriction: None, .. } => {
                Ok(self.bool_type.const_int(1, false))
            }
        }
    }

    /// Enlaza las variables introducidas por un patron en el scope actual.
    fn bind_match_pattern_vars(
        &mut self,
        pattern: &Pattern,
        scrut_slot: inkwell::values::PointerValue<'ctx>,
        scrut_kind: ValueKind,
        _scrut_sem_type: &SemanticType,
    ) -> Result<(), String> {
        match pattern {
            Pattern::Identifier { name, .. } => {
                // Crear un alloca nuevo para la variable de patron y copiar el escrutinio.
                let ptr = self.alloca_for_kind(&scrut_kind, name)?;
                let scrut_val =
                    self.load_value(&scrut_kind, scrut_slot, &format!("scrut_for_{}", name))?;
                self.store_value(ptr, scrut_val)?;
                self.insert_var(name.clone(), VarInfo { ptr, kind: scrut_kind });
                Ok(())
            }
            Pattern::Binary { left, right, .. } => {
                self.bind_match_pattern_vars(left, scrut_slot, scrut_kind, _scrut_sem_type)?;
                self.bind_match_pattern_vars(right, scrut_slot, scrut_kind, _scrut_sem_type)
            }
            Pattern::Unary { operand, .. } => {
                self.bind_match_pattern_vars(operand, scrut_slot, scrut_kind, _scrut_sem_type)
            }
            // Literales y Default no introducen variables.
            Pattern::Literal(_) | Pattern::Default => Ok(()),
        }
    }

    /// Carga el type_id almacenado en el primer campo de la vtable de un objeto.
    /// Equivalente a `load_type_id` en objects.rs, expuesto aquí para uso desde match.
    fn load_type_id_from_obj(
        &self,
        object_value: inkwell::values::PointerValue<'ctx>,
        static_type: &str,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        use inkwell::AddressSpace;

        let object_struct = self.object_struct_type(static_type)?;
        let typed_ptr = self.cast_object_ptr(&object_value, static_type)?;
        let vtable_slot = self
            .builder
            .build_struct_gep(object_struct, typed_ptr, 0, "vtable_slot_m")
            .map_err(|e| e.to_string())?;
        let vtable_ptr = self
            .builder
            .build_load(self.vtable_ptr_type(), vtable_slot, "vtable_ptr_m")
            .map_err(|e| e.to_string())?
            .into_pointer_value();
        let type_ptr = self
            .builder
            .build_pointer_cast(
                vtable_ptr,
                self.context.i64_type().ptr_type(AddressSpace::default()),
                "type_id_ptr_m",
            )
            .map_err(|e| e.to_string())?;
        let type_id = self
            .builder
            .build_load(self.context.i64_type(), type_ptr, "type_id_m")
            .map_err(|e| e.to_string())?
            .into_int_value();
        Ok(type_id)
    }

    /// Declara `strcmp` de libc si aún no está en el módulo.
    pub(super) fn get_strcmp_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("strcmp") {
            return f;
        }
        let i8_ptr = self
            .context
            .i8_type()
            .ptr_type(inkwell::AddressSpace::default());
        let fn_type = self.context.i32_type().fn_type(&[i8_ptr.into(), i8_ptr.into()], false);
        self.module.add_function("strcmp", fn_type, None)
    }
}
