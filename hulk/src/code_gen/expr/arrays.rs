use inkwell::AddressSpace;
use inkwell::types::BasicType;

use crate::ast::{ArrayExpr, IndexExpr, Expr, KindExpr, CallExpr};
use crate::ast::ArrayComprehensionExpr;
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::super::{CodeGenerator, CodegenValue, ValueKind};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_array(
        &mut self,
        expr: &Expr,
        array: &ArrayExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        // Determine element semantic type
        let elem_sem = if let Some(SemanticType::Vector(inner)) = analysis.inferred_types.get(&expr.id)
        {
            *inner.clone()
        } else if !array.elements.is_empty() {
            analysis
                .inferred_types
                .get(&array.elements[0].id)
                .cloned()
                .ok_or_else(|| "No se encontro el tipo inferido del elemento".to_string())?
        } else {
            return Err("No se pudo determinar el tipo de elementos del vector".to_string());
        };

        let elem_kind = self.value_kind_from_semantic(&elem_sem)?;

        // length
        let len = array.elements.len() as u64;
        let len_const = self.context.i64_type().const_int(len, false);

        // allocate vector struct
        let struct_size = self
            .vector_struct
            .size_of()
            .ok_or_else(|| "No se pudo calcular tamano del vector_struct".to_string())?;
        let malloc_fn = self.get_malloc_function();
        let struct_size_i64 = self
            .builder
            .build_int_cast(struct_size, self.context.i64_type(), "vec_struct_size")
            .map_err(|e| e.to_string())?;
        let raw_ptr = self
            .builder
            .build_call(malloc_fn, &[struct_size_i64.into()], "vec_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();

        let vec_typed = self
            .builder
            .build_pointer_cast(
                raw_ptr,
                self.vector_struct.ptr_type(AddressSpace::default()),
                "vec_typed",
            )
            .map_err(|e| e.to_string())?;

        // allocate data buffer
        let elem_basic = self.basic_type_for_semantic(&elem_sem)?;
        let elem_size = elem_basic
            .size_of()
            .ok_or_else(|| "No se pudo calcular tamano del elemento".to_string())?;
        let elem_size_i64 = self
            .builder
            .build_int_cast(elem_size, self.context.i64_type(), "elem_size")
            .map_err(|e| e.to_string())?;
        let total = self
            .builder
            .build_int_mul(elem_size_i64, len_const, "data_size")
            .map_err(|e| e.to_string())?;

        let raw_data = self
            .builder
            .build_call(malloc_fn, &[total.into()], "data_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();

        // cast raw_data to element pointer type
        let elem_ptr_type = match elem_basic {
            inkwell::types::BasicTypeEnum::FloatType(ft) => ft.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::IntType(it) => it.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::PointerType(pt) => pt.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::StructType(st) => st.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::ArrayType(at) => at.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::VectorType(vt) => vt.ptr_type(AddressSpace::default()),
        };

        let data_typed = self
            .builder
            .build_pointer_cast(raw_data, elem_ptr_type, "data_typed")
            .map_err(|e| e.to_string())?;

        // store length and data pointer into struct fields
        let len_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 0, "vec_len_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(len_slot, len_const)
            .map_err(|e| e.to_string())?;

        let data_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 1, "vec_data_slot")
            .map_err(|e| e.to_string())?;
        let data_cast = self
            .builder
            .build_pointer_cast(
                data_typed,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "data_cast",
            )
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(data_slot, data_cast)
            .map_err(|e| e.to_string())?;

        let cursor_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 2, "vec_cursor_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(cursor_slot, self.context.i64_type().const_int(u64::MAX, false))
            .map_err(|e| e.to_string())?;

        // fill elements
        for (i, elem_expr) in array.elements.iter().enumerate() {
            let value = self.lower_expr(elem_expr, analysis)?;

            let idx_val = self
                .context
                .i64_type()
                .const_int(i as u64, false);
            let byte_offset = self
                .builder
                .build_int_mul(idx_val, elem_size_i64, &format!("byte_offset_{}", i))
                .map_err(|e| e.to_string())?;

            let base_i8 = self
                .builder
                .build_pointer_cast(
                    data_typed,
                    self.context.i8_type().ptr_type(AddressSpace::default()),
                    &format!("data_i8_{}", i),
                )
                .map_err(|e| e.to_string())?;

            let elem_i8_ptr = unsafe {
                self.builder
                    .build_in_bounds_gep(
                        self.context.i8_type(),
                        base_i8,
                        &[byte_offset],
                        &format!("elem_i8_{}", i),
                    )
                    .map_err(|e| e.to_string())?
            };

            let elem_ptr = self
                .builder
                .build_pointer_cast(elem_i8_ptr, elem_ptr_type, &format!("elem_ptr_{}", i))
                .map_err(|e| e.to_string())?;

            match (elem_kind, value) {
                (ValueKind::Number, CodegenValue::Number(n)) => {
                    self.builder
                        .build_store(elem_ptr, n)
                        .map_err(|e| e.to_string())?;
                }
                (ValueKind::Bool, CodegenValue::Bool(b)) => {
                    self.builder
                        .build_store(elem_ptr, b)
                        .map_err(|e| e.to_string())?;
                }
                (ValueKind::String, CodegenValue::String(s))
                | (ValueKind::Object, CodegenValue::Object(s))
                | (ValueKind::Vector, CodegenValue::Vector(s))
                | (ValueKind::Closure, CodegenValue::Closure(s)) => {
                    self.builder
                        .build_store(elem_ptr, s)
                        .map_err(|e| e.to_string())?;
                }
                _ => return Err("Tipo de elemento y valor no coinciden al inicializar vector".to_string()),
            }
        }

        Ok(CodegenValue::Vector(vec_typed))
    }

    pub(super) fn lower_array_comprehension(
        &mut self,
        comp: &ArrayComprehensionExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        if let KindExpr::Call(call) = &comp.iterable.kind
            && let KindExpr::Variable(callee) = &call.callee.kind
            && callee.name == "range"
            && call.arguments.len() == 2
        {
            return self.lower_array_comprehension_range(comp, call, analysis);
        }

        // Evaluate iterable
        let iterable_val = self.lower_expr(&comp.iterable, analysis)?;
        let (src_ptr, elem_sem) = match iterable_val {
            CodegenValue::Vector(ptr) => {
                let Some(SemanticType::Vector(inner)) = analysis.inferred_types.get(&comp.iterable.id) else {
                    return Err("No se encontro el tipo inferido del iterable".to_string());
                };
                (ptr, *inner.clone())
            }
            _ => return Err("Array comprehension solo soporta iterables vectoriales".to_string()),
        };

        let elem_kind = self.value_kind_from_semantic(&elem_sem)?;

        // load source length and data pointer
        let len_slot = self
            .builder
            .build_struct_gep(self.vector_struct, src_ptr, 0, "src_len_slot")
            .map_err(|e| e.to_string())?;
        let len_val = self
            .builder
            .build_load(self.context.i64_type(), len_slot, "src_len")
            .map_err(|e| e.to_string())?
            .into_int_value();

        let src_data_slot = self
            .builder
            .build_struct_gep(self.vector_struct, src_ptr, 1, "src_data_slot")
            .map_err(|e| e.to_string())?;
        let src_data_i8 = self
            .builder
            .build_load(
                self.context.i8_type().ptr_type(AddressSpace::default()),
                src_data_slot,
                "src_data_ptr",
            )
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        // allocate dest vector struct
        let struct_size = self
            .vector_struct
            .size_of()
            .ok_or_else(|| "No se pudo calcular tamano del vector_struct".to_string())?;
        let malloc_fn = self.get_malloc_function();
        let struct_size_i64 = self
            .builder
            .build_int_cast(struct_size, self.context.i64_type(), "vec_struct_size")
            .map_err(|e| e.to_string())?;
        let raw_ptr = self
            .builder
            .build_call(malloc_fn, &[struct_size_i64.into()], "vec_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();

        let vec_typed = self
            .builder
            .build_pointer_cast(
                raw_ptr,
                self.vector_struct.ptr_type(AddressSpace::default()),
                "vec_typed",
            )
            .map_err(|e| e.to_string())?;

        // allocate data buffer: total = elem_size * len_val
        let elem_basic = self.basic_type_for_semantic(&elem_sem)?;
        let elem_size = elem_basic
            .size_of()
            .ok_or_else(|| "No se pudo calcular tamano del elemento".to_string())?;
        let elem_size_i64 = self
            .builder
            .build_int_cast(elem_size, self.context.i64_type(), "elem_size")
            .map_err(|e| e.to_string())?;

        let total = self
            .builder
            .build_int_mul(elem_size_i64, len_val, "data_size")
            .map_err(|e| e.to_string())?;

        let raw_data = self
            .builder
            .build_call(malloc_fn, &[total.into()], "data_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();

        // cast raw_data to element pointer type
        let elem_ptr_type = match elem_basic {
            inkwell::types::BasicTypeEnum::FloatType(ft) => ft.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::IntType(it) => it.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::PointerType(pt) => pt.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::StructType(st) => st.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::ArrayType(at) => at.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::VectorType(vt) => vt.ptr_type(AddressSpace::default()),
        };

        let data_typed = self
            .builder
            .build_pointer_cast(raw_data, elem_ptr_type, "data_typed")
            .map_err(|e| e.to_string())?;

        // store length and data pointer into dest struct
        let len_slot_dst = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 0, "dst_len_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(len_slot_dst, len_val)
            .map_err(|e| e.to_string())?;

        let data_slot_dst = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 1, "dst_data_slot")
            .map_err(|e| e.to_string())?;
        let data_cast = self
            .builder
            .build_pointer_cast(
                data_typed,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "data_cast",
            )
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(data_slot_dst, data_cast)
            .map_err(|e| e.to_string())?;

        let cursor_slot_dst = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 2, "dst_cursor_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(cursor_slot_dst, self.context.i64_type().const_int(u64::MAX, false))
            .map_err(|e| e.to_string())?;

        // Loop: for i in 0..len_val { src_elem = load(src, i); let var = src_elem; element_val = lower_expr(element); store(dst, i, element_val) }
        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| "No se encontro funcion actual".to_string())?;

        let loop_cond = self.context.append_basic_block(function, "comp_cond");
        let loop_body = self.context.append_basic_block(function, "comp_body");
        let loop_after = self.context.append_basic_block(function, "comp_after");

        // idx = 0
        let idx_ptr = self
            .builder
            .build_alloca(self.context.i64_type(), "comp_idx")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(idx_ptr, self.context.i64_type().const_int(0, false))
            .map_err(|e| e.to_string())?;

        self.builder
            .build_unconditional_branch(loop_cond)
            .map_err(|e| e.to_string())?;

        // cond
        self.builder.position_at_end(loop_cond);
        let idx_val = self
            .builder
            .build_load(self.context.i64_type(), idx_ptr, "idx_val")
            .map_err(|e| e.to_string())?
            .into_int_value();
        let cmp = self
            .builder
            .build_int_compare(inkwell::IntPredicate::ULT, idx_val, len_val, "cmp")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_conditional_branch(cmp, loop_body, loop_after)
            .map_err(|e| e.to_string())?;

        // body
        self.builder.position_at_end(loop_body);

        // compute source element pointer
        let byte_offset = self
            .builder
            .build_int_mul(idx_val, elem_size_i64, "byte_offset")
            .map_err(|e| e.to_string())?;

        let base_i8 = self
            .builder
            .build_pointer_cast(
                src_data_i8,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "src_data_i8",
            )
            .map_err(|e| e.to_string())?;

        let src_elem_i8 = unsafe {
            self.builder
                .build_in_bounds_gep(self.context.i8_type(), base_i8, &[byte_offset], "src_elem_i8")
                .map_err(|e| e.to_string())?
        };

        let src_elem_ptr = self
            .builder
            .build_pointer_cast(src_elem_i8, elem_ptr_type, "src_elem_ptr")
            .map_err(|e| e.to_string())?;

        // load source element value according to kind
        let src_value = match elem_kind {
            ValueKind::Number => CodegenValue::Number(
                self.builder
                    .build_load(self.f64_type, src_elem_ptr, "load_src_elem")
                    .map_err(|e| e.to_string())?
                    .into_float_value(),
            ),
            ValueKind::Bool => CodegenValue::Bool(
                self.builder
                    .build_load(self.bool_type, src_elem_ptr, "load_src_elem")
                    .map_err(|e| e.to_string())?
                    .into_int_value(),
            ),
            ValueKind::String => CodegenValue::String(
                self.builder
                    .build_load(
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        src_elem_ptr,
                        "load_src_elem",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            ValueKind::Object => CodegenValue::Object(
                self.builder
                    .build_load(
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        src_elem_ptr,
                        "load_src_elem",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            ValueKind::Vector => CodegenValue::Vector(
                self.builder
                    .build_load(
                        self.vector_struct.ptr_type(AddressSpace::default()),
                        src_elem_ptr,
                        "load_src_elem",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            ValueKind::Closure => CodegenValue::Closure(
                self.builder
                    .build_load(
                        self.closure_struct.ptr_type(AddressSpace::default()),
                        src_elem_ptr,
                        "load_src_elem",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
        };

        // bind iteration variable
        self.enter_scope();
        let var_slot = self.alloca_for_kind(&elem_kind, &comp.variable)?;
        self.store_value(var_slot, src_value)?;
        self.insert_var(
            comp.variable.clone(),
            super::super::VarInfo {
                ptr: var_slot,
                kind: elem_kind,
            },
        );

        // evaluate element expression
        let result = self.lower_expr(&comp.element, analysis)?;

        // store result into dest buffer at idx
        let dest_base_i8 = self
            .builder
            .build_pointer_cast(
                data_typed,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "dst_data_i8",
            )
            .map_err(|e| e.to_string())?;

        let dest_elem_i8 = unsafe {
            self.builder
                .build_in_bounds_gep(self.context.i8_type(), dest_base_i8, &[byte_offset], "dst_elem_i8")
                .map_err(|e| e.to_string())?
        };

        let dest_elem_ptr = self
            .builder
            .build_pointer_cast(dest_elem_i8, elem_ptr_type, "dst_elem_ptr")
            .map_err(|e| e.to_string())?;

        self.store_value(dest_elem_ptr, result)?;

        // cleanup loop variable
        self.exit_scope();

        // idx = idx + 1
        let next = self
            .builder
            .build_int_add(idx_val, self.context.i64_type().const_int(1, false), "idx_next")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(idx_ptr, next)
            .map_err(|e| e.to_string())?;

        self.builder
            .build_unconditional_branch(loop_cond)
            .map_err(|e| e.to_string())?;

        // after
        self.builder.position_at_end(loop_after);

        Ok(CodegenValue::Vector(vec_typed))
    }

    // Comprension con `range(start, end)` como fuente: `range` no es una funcion
    // real en el modulo (solo esta desazucarado para `for`), asi que aqui se
    // genera la secuencia directamente con un contador en lugar de leer de un
    // vector fuente, reusando la misma convencion de paso 1.0 que `lower_for_range`.
    fn lower_array_comprehension_range(
        &mut self,
        comp: &ArrayComprehensionExpr,
        call: &CallExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let start = self.lower_expr(&call.arguments[0], analysis)?.into_number()?;
        let end = self.lower_expr(&call.arguments[1], analysis)?.into_number()?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|b| b.get_parent())
            .ok_or_else(|| "No se encontro funcion actual".to_string())?;

        // Primera pasada: contar cuantas iteraciones produce range(start, end)
        // con paso 1.0, igual que lower_for_range, para poder reservar el
        // buffer del vector destino de una sola vez.
        let counter_ptr = self
            .builder
            .build_alloca(self.f64_type, "comp_range_counter")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(counter_ptr, start)
            .map_err(|e| e.to_string())?;
        let count_ptr = self
            .builder
            .build_alloca(self.context.i64_type(), "comp_range_count")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(count_ptr, self.context.i64_type().const_int(0, false))
            .map_err(|e| e.to_string())?;

        let count_cond = self.context.append_basic_block(function, "comp_range_count_cond");
        let count_body = self.context.append_basic_block(function, "comp_range_count_body");
        let count_after = self.context.append_basic_block(function, "comp_range_count_after");

        self.builder
            .build_unconditional_branch(count_cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(count_cond);
        let counter_value = self
            .builder
            .build_load(self.f64_type, counter_ptr, "comp_range_counter_load")
            .map_err(|e| e.to_string())?
            .into_float_value();
        let count_cmp = self
            .builder
            .build_float_compare(inkwell::FloatPredicate::OLT, counter_value, end, "comp_range_count_cmp")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_conditional_branch(count_cmp, count_body, count_after)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(count_body);
        let count_value = self
            .builder
            .build_load(self.context.i64_type(), count_ptr, "comp_range_count_load")
            .map_err(|e| e.to_string())?
            .into_int_value();
        let count_next = self
            .builder
            .build_int_add(count_value, self.context.i64_type().const_int(1, false), "comp_range_count_next")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(count_ptr, count_next)
            .map_err(|e| e.to_string())?;
        let counter_next = self
            .builder
            .build_float_add(counter_value, self.f64_type.const_float(1.0), "comp_range_counter_next")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(counter_ptr, counter_next)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(count_cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(count_after);
        let len_val = self
            .builder
            .build_load(self.context.i64_type(), count_ptr, "comp_range_len")
            .map_err(|e| e.to_string())?
            .into_int_value();

        // El elemento de range(...) siempre es Number
        let elem_kind = ValueKind::Number;
        let elem_size = self
            .f64_type
            .size_of();
        let elem_size_i64 = self
            .builder
            .build_int_cast(elem_size, self.context.i64_type(), "comp_range_elem_size")
            .map_err(|e| e.to_string())?;

        // Reservar struct y buffer del vector destino
        let struct_size = self
            .vector_struct
            .size_of()
            .ok_or_else(|| "No se pudo calcular tamano del vector_struct".to_string())?;
        let malloc_fn = self.get_malloc_function();
        let struct_size_i64 = self
            .builder
            .build_int_cast(struct_size, self.context.i64_type(), "vec_struct_size")
            .map_err(|e| e.to_string())?;
        let raw_ptr = self
            .builder
            .build_call(malloc_fn, &[struct_size_i64.into()], "vec_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();
        let vec_typed = self
            .builder
            .build_pointer_cast(
                raw_ptr,
                self.vector_struct.ptr_type(AddressSpace::default()),
                "vec_typed",
            )
            .map_err(|e| e.to_string())?;

        let total = self
            .builder
            .build_int_mul(elem_size_i64, len_val, "data_size")
            .map_err(|e| e.to_string())?;
        let raw_data = self
            .builder
            .build_call(malloc_fn, &[total.into()], "data_alloc")
            .map_err(|e| e.to_string())?
            .try_as_basic_value()
            .left()
            .ok_or_else(|| "malloc no devolvio un valor".to_string())?
            .into_pointer_value();
        let data_typed = self
            .builder
            .build_pointer_cast(raw_data, self.f64_type.ptr_type(AddressSpace::default()), "data_typed")
            .map_err(|e| e.to_string())?;

        let len_slot_dst = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 0, "dst_len_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(len_slot_dst, len_val)
            .map_err(|e| e.to_string())?;
        let data_slot_dst = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 1, "dst_data_slot")
            .map_err(|e| e.to_string())?;
        let data_cast = self
            .builder
            .build_pointer_cast(
                data_typed,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "data_cast",
            )
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(data_slot_dst, data_cast)
            .map_err(|e| e.to_string())?;
        let cursor_slot_dst = self
            .builder
            .build_struct_gep(self.vector_struct, vec_typed, 2, "dst_cursor_slot")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(cursor_slot_dst, self.context.i64_type().const_int(u64::MAX, false))
            .map_err(|e| e.to_string())?;

        // Segunda pasada: generar cada elemento como `start + idx` y evaluar
        // la expresion del comprehension con la variable enlazada a ese valor.
        let idx_ptr = self
            .builder
            .build_alloca(self.context.i64_type(), "comp_idx")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(idx_ptr, self.context.i64_type().const_int(0, false))
            .map_err(|e| e.to_string())?;

        let loop_cond = self.context.append_basic_block(function, "comp_range_cond");
        let loop_body = self.context.append_basic_block(function, "comp_range_body");
        let loop_after = self.context.append_basic_block(function, "comp_range_after");

        self.builder
            .build_unconditional_branch(loop_cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(loop_cond);
        let idx_val = self
            .builder
            .build_load(self.context.i64_type(), idx_ptr, "idx_val")
            .map_err(|e| e.to_string())?
            .into_int_value();
        let cmp = self
            .builder
            .build_int_compare(inkwell::IntPredicate::ULT, idx_val, len_val, "cmp")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_conditional_branch(cmp, loop_body, loop_after)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(loop_body);
        let idx_as_f64 = self
            .builder
            .build_signed_int_to_float(idx_val, self.f64_type, "idx_as_f64")
            .map_err(|e| e.to_string())?;
        let src_value = CodegenValue::Number(
            self.builder
                .build_float_add(start, idx_as_f64, "comp_range_elem")
                .map_err(|e| e.to_string())?,
        );

        self.enter_scope();
        let var_slot = self.alloca_for_kind(&elem_kind, &comp.variable)?;
        self.store_value(var_slot, src_value)?;
        self.insert_var(
            comp.variable.clone(),
            super::super::VarInfo {
                ptr: var_slot,
                kind: elem_kind,
            },
        );

        let result = self.lower_expr(&comp.element, analysis)?;

        let byte_offset = self
            .builder
            .build_int_mul(idx_val, elem_size_i64, "byte_offset")
            .map_err(|e| e.to_string())?;
        let dest_base_i8 = self
            .builder
            .build_pointer_cast(
                data_typed,
                self.context.i8_type().ptr_type(AddressSpace::default()),
                "dst_data_i8",
            )
            .map_err(|e| e.to_string())?;
        let dest_elem_i8 = unsafe {
            self.builder
                .build_in_bounds_gep(self.context.i8_type(), dest_base_i8, &[byte_offset], "dst_elem_i8")
                .map_err(|e| e.to_string())?
        };
        let dest_elem_ptr = self
            .builder
            .build_pointer_cast(dest_elem_i8, self.f64_type.ptr_type(AddressSpace::default()), "dst_elem_ptr")
            .map_err(|e| e.to_string())?;
        self.store_value(dest_elem_ptr, result)?;

        self.exit_scope();

        let next = self
            .builder
            .build_int_add(idx_val, self.context.i64_type().const_int(1, false), "idx_next")
            .map_err(|e| e.to_string())?;
        self.builder
            .build_store(idx_ptr, next)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_unconditional_branch(loop_cond)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(loop_after);

        Ok(CodegenValue::Vector(vec_typed))
    }

    pub(super) fn lower_index(
        &mut self,
        index: &IndexExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let obj = self.lower_expr(&index.object, analysis)?;
        let (vec_ptr, elem_sem) = match obj {
            CodegenValue::Vector(ptr) => {
                // find element semantic type
                let Some(SemanticType::Vector(inner)) = analysis.inferred_types.get(&index.object.id) else {
                    return Err("No se encontro el tipo inferido del vector".to_string());
                };
                (ptr, *inner.clone())
            }
            _ => return Err("Indexing solo soportado en vectores".to_string()),
        };

        let elem_kind = self.value_kind_from_semantic(&elem_sem)?;

        // load data pointer
        let data_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_ptr, 1, "vec_data_slot")
            .map_err(|e| e.to_string())?;
        let data_i8 = self
            .builder
            .build_load(self.context.i8_type().ptr_type(AddressSpace::default()), data_slot, "data_ptr")
            .map_err(|e| e.to_string())?
            .into_pointer_value();

        // compute index
        let idx_val = self.lower_expr(&index.index, analysis)?;
        let idx_i64 = match idx_val {
            CodegenValue::Number(n) => self
                .builder
                .build_float_to_signed_int(n, self.context.i64_type(), "idx_i64")
                .map_err(|e| e.to_string())?,
            _ => return Err("Indice debe ser Number".to_string()),
        };

        let zero = self.context.i64_type().const_int(0, false);
        let in_range = self
            .builder
            .build_int_compare(inkwell::IntPredicate::SGE, idx_i64, zero, "idx_non_negative")
            .map_err(|e| e.to_string())?;

        let len_slot = self
            .builder
            .build_struct_gep(self.vector_struct, vec_ptr, 0, "vec_len_slot")
            .map_err(|e| e.to_string())?;
        let len_val = self
            .builder
            .build_load(self.context.i64_type(), len_slot, "vec_len")
            .map_err(|e| e.to_string())?
            .into_int_value();

        let lt_len = self
            .builder
            .build_int_compare(inkwell::IntPredicate::ULT, idx_i64, len_val, "idx_lt_len")
            .map_err(|e| e.to_string())?;

        let valid = self
            .builder
            .build_and(in_range, lt_len, "idx_valid")
            .map_err(|e| e.to_string())?;

        let function = self
            .builder
            .get_insert_block()
            .and_then(|block| block.get_parent())
            .ok_or_else(|| "No se pudo determinar la funcion actual para index".to_string())?;
        let ok_block = self.context.append_basic_block(function, "index_ok");
        let fail_block = self.context.append_basic_block(function, "index_fail");
        let cont_block = self.context.append_basic_block(function, "index_cont");

        self.builder
            .build_conditional_branch(valid, ok_block, fail_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(fail_block);
        let panic_fn = self.get_panic_function();
        let msg_name = self.fresh_tmp("index_panic_msg");
        let msg = self
            .builder
            .build_global_string_ptr("Runtime error: index out of bounds", &msg_name)
            .map_err(|e| e.to_string())?;
        self.builder
            .build_call(panic_fn, &[msg.as_pointer_value().into()], "index_panic")
            .map_err(|e| e.to_string())?;
        self.builder.build_unreachable().map_err(|e| e.to_string())?;

        self.builder.position_at_end(ok_block);

        // element size
        let elem_basic = self.basic_type_for_semantic(&elem_sem)?;
        let elem_size = elem_basic
            .size_of()
            .ok_or_else(|| "No se pudo obtener tamano de elemento".to_string())?;
        let elem_size_i64 = self
            .builder
            .build_int_cast(elem_size, self.context.i64_type(), "elem_size")
            .map_err(|e| e.to_string())?;

        let byte_offset = self
            .builder
            .build_int_mul(idx_i64, elem_size_i64, "byte_offset")
            .map_err(|e| e.to_string())?;

        let elem_i8_ptr = unsafe {
            self.builder
                .build_in_bounds_gep(
                    self.context.i8_type(),
                    data_i8,
                    &[byte_offset],
                    "elem_i8",
                )
                .map_err(|e| e.to_string())?
        };

        let elem_ptr_type = match elem_basic {
            inkwell::types::BasicTypeEnum::FloatType(ft) => ft.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::IntType(it) => it.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::PointerType(pt) => pt.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::StructType(st) => st.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::ArrayType(at) => at.ptr_type(AddressSpace::default()),
            inkwell::types::BasicTypeEnum::VectorType(vt) => vt.ptr_type(AddressSpace::default()),
        };

        let elem_ptr = self
            .builder
            .build_pointer_cast(elem_i8_ptr, elem_ptr_type, "elem_ptr")
            .map_err(|e| e.to_string())?;

        let loaded = match elem_kind {
            ValueKind::Number => CodegenValue::Number(
                self.builder
                    .build_load(self.f64_type, elem_ptr, "load_elem")
                    .map_err(|e| e.to_string())?
                    .into_float_value(),
            ),
            ValueKind::Bool => CodegenValue::Bool(
                self.builder
                    .build_load(self.bool_type, elem_ptr, "load_elem")
                    .map_err(|e| e.to_string())?
                    .into_int_value(),
            ),
            ValueKind::String => CodegenValue::String(
                self.builder
                    .build_load(
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "load_elem",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            ValueKind::Object => CodegenValue::Object(
                self.builder
                    .build_load(
                        self.context.i8_type().ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "load_elem",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            ValueKind::Vector => CodegenValue::Vector(
                self.builder
                    .build_load(
                        self.vector_struct.ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "load_elem",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
            ValueKind::Closure => CodegenValue::Closure(
                self.builder
                    .build_load(
                        self.closure_struct.ptr_type(AddressSpace::default()),
                        elem_ptr,
                        "load_elem",
                    )
                    .map_err(|e| e.to_string())?
                    .into_pointer_value(),
            ),
        };

        self.builder
            .build_unconditional_branch(cont_block)
            .map_err(|e| e.to_string())?;

        self.builder.position_at_end(cont_block);

        Ok(loaded)
    }
}
