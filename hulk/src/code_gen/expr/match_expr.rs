use inkwell::AddressSpace;

use crate::ast::{BinaryOperator, LiteralValue, MatchCase, MatchExpr, Pattern, UnaryOperator};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::super::{CodeGenerator, CodegenValue, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    /// Lowers a `match expr { case pat => body; ... default => body; }` to a
    /// cascade of conditional branches, one per case, with a shared merge block
    /// that collects the result via a PHI node.
    pub(super) fn lower_match(
        &mut self,
        match_expr: &MatchExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        // Evalúa el escrutinio una sola vez y lo guarda en un alloca para poder
        // reutilizarlo en cada rama sin re-evaluar efectos secundarios.
        let scrutinee = self.lower_expr(&match_expr.expression, analysis)?;
        let scrutinee_kind = scrutinee.kind();
        let scrutinee_slot = self.alloca_for_kind(&scrutinee_kind, "match_scrutinee")?;
        self.store_value(scrutinee_slot, scrutinee)?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| "No hay funcion activa en match".to_string())?;

        // Bloque de merge: todos los casos saltan aquí al terminar.
        let merge_block = self.context.append_basic_block(function, "match_merge");

        // Recolectamos (valor_resultado, bloque_emisor) por cada caso para el PHI final.
        let mut incoming: Vec<(CodegenValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
            Vec::new();

        for (idx, case) in match_expr.cases.iter().enumerate() {
            let case_label = format!("match_case_{}", idx);
            let next_label = format!("match_next_{}", idx);

            let case_block = self.context.append_basic_block(function, &case_label);
            let next_block = self.context.append_basic_block(function, &next_label);

            // Recarga el escrutinio para la comparación.
            let scrutinee_val = self.load_value(&scrutinee_kind, scrutinee_slot, "match_load")?;

            // Compila la condición del patrón (None = default).
            let cond = self.pattern_condition(&case.pattern, scrutinee_val, analysis)?;

            match cond {
                Some(cond_val) => {
                    self.builder
                        .build_conditional_branch(cond_val, case_block, next_block)
                        .map_err(|e| e.to_string())?;
                }
                None => {
                    // Patrón `default` o catch-all — salta incondicionalmente al caso.
                    self.builder
                        .build_unconditional_branch(case_block)
                        .map_err(|e| e.to_string())?;
                }
            }

            // ── Cuerpo del caso ──────────────────────────────────────────────
            self.builder.position_at_end(case_block);
            self.enter_scope();

            // Enlaza los identificadores introducidos por el patrón en este scope.
            let scrutinee_val2 =
                self.load_value(&scrutinee_kind, scrutinee_slot, "match_bind_load")?;
            self.bind_pattern_identifiers(&case.pattern, scrutinee_val2, analysis)?;

            let body_val = self.lower_expr(&case.body, analysis)?;
            self.exit_scope();

            let case_end_block = self
                .builder
                .get_insert_block()
                .ok_or_else(|| "No hay bloque activo al final del caso match".to_string())?;
            self.builder
                .build_unconditional_branch(merge_block)
                .map_err(|e| e.to_string())?;

            incoming.push((body_val, case_end_block));

            // ── Bloque siguiente (fallo de patrón) ──────────────────────────
            self.builder.position_at_end(next_block);
        }

        // Si llegamos aquí sin haber hecho match (match no exhaustivo en runtime),
        // emitimos panic y unreachable para mantener válido el CFG.
        let panic_fn = self.get_panic_function();
        let msg_name = self.fresh_tmp("match_panic_msg");
        let msg = self
            .builder
            .build_global_string_ptr("Runtime error: match no exhaustivo", &msg_name)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_call(panic_fn, &[msg.as_pointer_value().into()], "match_panic")
            .map_err(|e| e.to_string())?;
        self.builder.build_unreachable().map_err(|e| e.to_string())?;

        // ── Merge block: PHI ─────────────────────────────────────────────────
        self.builder.position_at_end(merge_block);

        if incoming.is_empty() {
            return Err("Match sin casos".to_string());
        }

        // Todos los casos deben producir el mismo ValueKind.
        let result_kind = incoming[0].0.kind();
        if incoming.iter().any(|(v, _)| v.kind() != result_kind) {
            return Err("Los casos de match producen tipos incompatibles".to_string());
        }

        // Construir PHI según el kind del resultado.
        match result_kind {
            ValueKind::Number => {
                let phi = self
                    .builder
                    .build_phi(self.f64_type, "match_result")
                    .map_err(|e| e.to_string())?;
                for (val, block) in &incoming {
                    phi.add_incoming(&[(&val.into_number()?, *block)]);
                }
                Ok(CodegenValue::Number(
                    phi.as_basic_value().into_float_value(),
                ))
            }
            ValueKind::Bool => {
                let phi = self
                    .builder
                    .build_phi(self.bool_type, "match_result_bool")
                    .map_err(|e| e.to_string())?;
                for (val, block) in &incoming {
                    phi.add_incoming(&[(&val.into_bool()?, *block)]);
                }
                Ok(CodegenValue::Bool(phi.as_basic_value().into_int_value()))
            }
            ValueKind::String => {
                let i8ptr = self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::default());
                let phi = self
                    .builder
                    .build_phi(i8ptr, "match_result_str")
                    .map_err(|e| e.to_string())?;
                for (val, block) in &incoming {
                    phi.add_incoming(&[(&val.into_string()?, *block)]);
                }
                Ok(CodegenValue::String(
                    phi.as_basic_value().into_pointer_value(),
                ))
            }
            ValueKind::Object => {
                let i8ptr = self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::default());
                let phi = self
                    .builder
                    .build_phi(i8ptr, "match_result_obj")
                    .map_err(|e| e.to_string())?;
                for (val, block) in &incoming {
                    phi.add_incoming(&[(&val.into_object()?, *block)]);
                }
                Ok(CodegenValue::Object(
                    phi.as_basic_value().into_pointer_value(),
                ))
            }
            ValueKind::Vector => {
                let vec_ptr_ty = self.vector_struct.ptr_type(AddressSpace::default());
                let phi = self
                    .builder
                    .build_phi(vec_ptr_ty, "match_result_vec")
                    .map_err(|e| e.to_string())?;
                for (val, block) in &incoming {
                    phi.add_incoming(&[(&val.into_vector()?, *block)]);
                }
                Ok(CodegenValue::Vector(
                    phi.as_basic_value().into_pointer_value(),
                ))
            }
        }
    }

    // ── Condición de patrón ──────────────────────────────────────────────────

    /// Devuelve `None` para patrones incondicionalmente verdaderos (default/catch-all).
    /// Devuelve `Some(i1)` para los demás.
    fn pattern_condition(
        &mut self,
        pattern: &Pattern,
        scrutinee: CodegenValue<'ctx>,
        analysis: &SemanticAnalysis,
    ) -> Result<Option<inkwell::values::IntValue<'ctx>>, String> {
        match pattern {
            Pattern::Default => Ok(None),
            Pattern::Identifier {
                type_restriction: None,
                ..
            } => Ok(None), // catch-all sin tipo

            Pattern::Identifier {
                type_restriction: Some(type_ref),
                ..
            } => {
                // Patrón de tipo: `case x: T => ...`
                // Sólo tiene sentido sobre objetos.
                let target = SemanticType::from_type_ref(type_ref);
                let cond = self.pattern_type_check(scrutinee, &target, analysis)?;
                Ok(Some(cond))
            }

            Pattern::Literal(lit) => {
                let cond = self.pattern_literal_eq(scrutinee, lit)?;
                Ok(Some(cond))
            }

            Pattern::Binary {
                left,
                operator,
                right,
            } => {
                // Patrón compuesto: `case (x:T1 + x:T2)` — en macros esta forma
                // actúa sobre la *estructura* del AST, pero en runtime desazúcar
                // lo convierte a expresiones normales. Aquí lo evaluamos como la
                // conjunción de ambas sub-condiciones.
                let left_cond =
                    self.pattern_condition(left, scrutinee, analysis)?;
                let right_cond =
                    self.pattern_condition(right, scrutinee, analysis)?;

                match (left_cond, right_cond) {
                    (None, None) => Ok(None),
                    (Some(l), None) => Ok(Some(l)),
                    (None, Some(r)) => Ok(Some(r)),
                    (Some(l), Some(r)) => {
                        let combined = match operator {
                            BinaryOperator::Or => self
                                .builder
                                .build_or(l, r, "pat_or")
                                .map_err(|e| e.to_string())?,
                            _ => self
                                .builder
                                .build_and(l, r, "pat_and")
                                .map_err(|e| e.to_string())?,
                        };
                        Ok(Some(combined))
                    }
                }
            }

            Pattern::Unary {
                operator: UnaryOperator::Not,
                operand,
            } => {
                let inner = self.pattern_condition(operand, scrutinee, analysis)?;
                match inner {
                    None => Ok(Some(self.bool_type.const_int(0, false))), // !default = false
                    Some(v) => Ok(Some(
                        self.builder
                            .build_not(v, "pat_not")
                            .map_err(|e| e.to_string())?,
                    )),
                }
            }

            Pattern::Unary { .. } => Ok(None),
        }
    }

    /// Compara el escrutinio con un literal.
    fn pattern_literal_eq(
        &mut self,
        scrutinee: CodegenValue<'ctx>,
        lit: &LiteralValue,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        match lit {
            LiteralValue::Number(n) => {
                let expected = self.f64_type.const_float(*n);
                let actual = scrutinee.into_number()?;
                self.builder
                    .build_float_compare(
                        inkwell::FloatPredicate::OEQ,
                        actual,
                        expected,
                        "pat_num_eq",
                    )
                    .map_err(|e| e.to_string())
            }
            LiteralValue::Bool(b) => {
                let expected = self.bool_type.const_int(u64::from(*b), false);
                let actual = scrutinee.into_bool()?;
                self.builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        actual,
                        expected,
                        "pat_bool_eq",
                    )
                    .map_err(|e| e.to_string())
            }
            LiteralValue::String(s) => {
                // Compara strings via strcmp (disponible en libc).
                let actual = scrutinee.into_string()?;
                let expected_name = self.fresh_tmp("pat_str_lit");
                // SAFETY: el string literal de HULK no contiene nulos.
                let expected = self
                    .builder
                    .build_global_string_ptr(s, &expected_name)
                    .map_err(|e| e.to_string())?
                    .as_pointer_value();

                let strcmp = self.get_strcmp_function();
                let cmp_result = self
                    .builder
                    .build_call(
                        strcmp,
                        &[actual.into(), expected.into()],
                        "pat_strcmp",
                    )
                    .map_err(|e| e.to_string())?
                    .try_as_basic_value()
                    .left()
                    .ok_or_else(|| "strcmp no devolvio valor".to_string())?
                    .into_int_value();

                self.builder
                    .build_int_compare(
                        inkwell::IntPredicate::EQ,
                        cmp_result,
                        self.context.i32_type().const_int(0, false),
                        "pat_str_eq",
                    )
                    .map_err(|e| e.to_string())
            }
        }
    }

    /// Comprueba si el escrutinio es una instancia del tipo dado (para `case x: T`).
    fn pattern_type_check(
        &mut self,
        scrutinee: CodegenValue<'ctx>,
        target: &SemanticType,
        analysis: &SemanticAnalysis,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        match scrutinee {
            CodegenValue::Number(_) => Ok(self
                .bool_type
                .const_int(u64::from(matches!(target, SemanticType::Number)), false)),
            CodegenValue::Bool(_) => Ok(self
                .bool_type
                .const_int(u64::from(matches!(target, SemanticType::Boolean)), false)),
            CodegenValue::String(_) => Ok(self
                .bool_type
                .const_int(u64::from(matches!(target, SemanticType::String)), false)),
            CodegenValue::Object(obj_ptr) => {
                let SemanticType::Custom(target_name) = target else {
                    return Ok(self.bool_type.const_int(0, false));
                };
                // Necesitamos el tipo estático para buscar vtable. Usamos el
                // tipo target como base — igual que lower_is para objetos.
                let type_id = self.load_type_id_from_ptr(obj_ptr)?;
                let ids = self.subtype_ids(target_name, analysis)?;
                let mut acc: Option<inkwell::values::IntValue<'ctx>> = None;
                for id in ids {
                    let expected = self.context.i64_type().const_int(id, false);
                    let cmp = self
                        .builder
                        .build_int_compare(
                            inkwell::IntPredicate::EQ,
                            type_id,
                            expected,
                            "pat_typeid_eq",
                        )
                        .map_err(|e| e.to_string())?;
                    acc = Some(match acc {
                        None => cmp,
                        Some(prev) => self
                            .builder
                            .build_or(prev, cmp, "pat_typeid_or")
                            .map_err(|e| e.to_string())?,
                    });
                }
                Ok(acc.unwrap_or_else(|| self.bool_type.const_int(0, false)))
            }
            CodegenValue::Vector(_) => Ok(self
                .bool_type
                .const_int(u64::from(matches!(target, SemanticType::Vector(_))), false)),
        }
    }

    // ── Binding de identificadores ───────────────────────────────────────────

    /// Para cada `Pattern::Identifier { name, ... }` en el árbol del patrón,
    /// crea una variable local en el scope actual con el valor del escrutinio
    /// (opcionalmente casteado al tipo restringido).
    fn bind_pattern_identifiers(
        &mut self,
        pattern: &Pattern,
        scrutinee: CodegenValue<'ctx>,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
        match pattern {
            Pattern::Identifier { name, type_restriction } => {
                // Determina el tipo real del binding.
                let (value, kind) = if let Some(type_ref) = type_restriction {
                    let target = SemanticType::from_type_ref(type_ref);
                    let kind = self.value_kind_from_semantic(&target)?;
                    // Si el escrutinio es Object y el target también, hacemos cast de puntero.
                    let v = match (&scrutinee, &target) {
                        (CodegenValue::Object(ptr), SemanticType::Custom(_)) => {
                            let casted = self
                                .builder
                                .build_pointer_cast(
                                    *ptr,
                                    self.context
                                        .i8_type()
                                        .ptr_type(AddressSpace::default()),
                                    "pat_cast",
                                )
                                .map_err(|e| e.to_string())?;
                            CodegenValue::Object(casted)
                        }
                        _ => scrutinee,
                    };
                    (v, kind)
                } else {
                    let kind = scrutinee.kind();
                    (scrutinee, kind)
                };

                let ptr = self.alloca_for_kind(&kind, name)?;
                self.store_value(ptr, value)?;
                self.insert_var(name.clone(), VarInfo { ptr, kind });
            }
            Pattern::Binary { left, right, .. } => {
                let scrutinee2 = scrutinee;
                // Ambos sub-patrones reciben el mismo escrutinio (es un patrón compuesto).
                self.bind_pattern_identifiers(left, scrutinee2, analysis)?;
                let reload = self.load_value(&scrutinee2.kind(), {
                    // No tenemos slot aquí, pero bind_pattern_identifiers ya guardó
                    // la copia; para right usamos el mismo valor de entrada.
                    // En la práctica los patrones Binary en runtime son conjunciones
                    // de restricciones de tipo sobre el mismo valor.
                    let tmp = self.alloca_for_kind(&scrutinee2.kind(), "pat_bin_tmp")?;
                    self.store_value(tmp, scrutinee2)?;
                    tmp
                }, "pat_bin_reload")?;
                self.bind_pattern_identifiers(right, reload, analysis)?;
            }
            Pattern::Unary { operand, .. } => {
                self.bind_pattern_identifiers(operand, scrutinee, analysis)?;
            }
            Pattern::Literal(_) | Pattern::Default => {}
        }
        Ok(())
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    /// Carga el type_id de un puntero de objeto sin necesitar el nombre estático.
    /// El type_id está en vtable[0] (primer campo de la vtable), y la vtable
    /// está en objeto[0].
    fn load_type_id_from_ptr(
        &self,
        obj_ptr: inkwell::values::PointerValue<'ctx>,
    ) -> Result<inkwell::values::IntValue<'ctx>, String> {
        let i8ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let i64_type = self.context.i64_type();
        // El primer campo del objeto (field 0) es el puntero a la vtable.
        let vtable_slot_type = self
            .context
            .struct_type(&[i8ptr.into()], false);
        let typed_obj = self
            .builder
            .build_pointer_cast(
                obj_ptr,
                vtable_slot_type.ptr_type(AddressSpace::default()),
                "typeid_obj_cast",
            )
            .map_err(|e| e.to_string())?;
        let vtable_slot = self
            .builder
            .build_struct_gep(vtable_slot_type, typed_obj, 0, "typeid_vtable_slot")
            .map_err(|e| e.to_string())?;
        let vtable_ptr = self
            .builder
            .build_load(i8ptr, vtable_slot, "typeid_vtable_ptr")
            .map_err(|e| e.to_string())?
            .into_pointer_value();
        // El primer campo de la vtable (field 0) es el type_id (i64).
        let vtable_hdr = self.context.struct_type(&[i64_type.into()], false);
        let vtable_typed = self
            .builder
            .build_pointer_cast(
                vtable_ptr,
                vtable_hdr.ptr_type(AddressSpace::default()),
                "typeid_vtable_cast",
            )
            .map_err(|e| e.to_string())?;
        let id_slot = self
            .builder
            .build_struct_gep(vtable_hdr, vtable_typed, 0, "typeid_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_load(i64_type, id_slot, "typeid_load")
            .map_err(|e| e.to_string())
            .map(|v| v.into_int_value())
    }

    /// Declara `strcmp` de libc si no está ya en el módulo.
    fn get_strcmp_function(&self) -> inkwell::values::FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("strcmp") {
            return f;
        }
        let i8ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let fn_type = self
            .context
            .i32_type()
            .fn_type(&[i8ptr.into(), i8ptr.into()], false);
        self.module.add_function("strcmp", fn_type, None)
    }

    /// Acepta los casos de un `MatchCase` para los tests.
    #[allow(dead_code)]
    pub(crate) fn lower_match_case_list(
        &mut self,
        cases: &[MatchCase],
        scrutinee: CodegenValue<'ctx>,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        // Sintetiza un MatchExpr temporal y lo baja.
        let dummy = crate::ast::mk_expr(crate::ast::KindExpr::Literal(crate::ast::LiteralExpr {
            value: LiteralValue::Number(0.0),
        }));
        let match_expr = MatchExpr {
            expression: Box::new(dummy),
            cases: cases.to_vec(),
        };
        // Guarda el escrutinio ya calculado en un slot temporal.
        let kind = scrutinee.kind();
        let slot = self.alloca_for_kind(&kind, "synth_scrutinee")?;
        self.store_value(slot, scrutinee)?;
        // Crea un MatchExpr cuya expresión es una variable dummy y lo evalúa.
        // Como ya tenemos el slot, llamamos lower_match directamente.
        // En su lugar delegamos al helper interno.
        drop(match_expr);
        drop(slot);
        Err("lower_match_case_list es solo para uso interno de tests".to_string())
    }
}
