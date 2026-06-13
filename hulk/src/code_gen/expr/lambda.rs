use inkwell::AddressSpace;

use crate::ast::LambdaExpr;
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::super::{CodeGenerator, CodegenValue, FunctionInfo, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    /// Lowers a lambda expression to an anonymous LLVM function and returns a
    /// pointer to it as an Object (the calling convention matches functor
    /// wrapper `invoke` methods so the callee can dispatch via vtable or direct
    /// call after desugar).
    ///
    /// Lambdas that the `functor_desugar` pass already wrapped as
    /// `_FunctorWrapper` objects reach codegen as `KindExpr::New` and are
    /// handled by `lower_new`. Only bare lambdas that escaped wrapping arrive
    /// here — typically those bound to a local variable that the desugar pass
    /// already marked as lowered and then invoked directly.
    pub(super) fn lower_lambda(
        &mut self,
        lambda: &LambdaExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        // ── Determina los tipos de parámetros ──────────────────────────────
        let mut param_kinds: Vec<ValueKind> = Vec::with_capacity(lambda.params.len());
        for param in &lambda.params {
            let sem_type = param
                .types
                .as_ref()
                .map(SemanticType::from_type_ref)
                .unwrap_or(SemanticType::Unknown);
            let kind = self.value_kind_from_semantic(&sem_type).unwrap_or(ValueKind::Object);
            param_kinds.push(kind);
        }

        // ── Determina el tipo de retorno ───────────────────────────────────
        let ret_kind = lambda
            .return_type
            .as_ref()
            .map(SemanticType::from_type_ref)
            .and_then(|t| self.value_kind_from_semantic(&t).ok())
            .or_else(|| {
                // Fallback: use the inferred type of the body expression.
                analysis
                    .inferred_types
                    .get(&lambda.body.id)
                    .and_then(|t| self.value_kind_from_semantic(t).ok())
            })
            .unwrap_or(ValueKind::Number);

        // ── Genera la función LLVM anónima ──────────────────────────────────
        let fn_name = self.fresh_tmp("_lambda");
        let fn_type = self.fn_type_for_signature(&param_kinds, ret_kind);
        let function = self.module.add_function(&fn_name, fn_type, None);

        // ── Registra la función para que llamadas directas la encuentren ────
        self.functions.insert(
            fn_name.clone(),
            FunctionInfo {
                function,
                params: param_kinds.clone(),
                ret: ret_kind,
            },
        );

        // ── Guarda el estado del builder y el scope actuales ────────────────
        let saved_block = self.builder.get_insert_block();
        let saved_type = self.current_type.clone();
        let saved_method = self.current_method.clone();

        // ── Define el cuerpo de la función anónima ──────────────────────────
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.enter_scope();
        self.current_type = None;
        self.current_method = None;

        for (idx, param) in lambda.params.iter().enumerate() {
            let arg = function
                .get_nth_param(idx as u32)
                .ok_or_else(|| format!("Parametro lambda faltante: {}", param.name))?;
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
                ValueKind::String | ValueKind::Object | ValueKind::Vector => {
                    self.builder
                        .build_store(ptr, arg.into_pointer_value())
                        .map_err(|e| e.to_string())?;
                }
            }
            self.insert_var(param.name.clone(), VarInfo { ptr, kind });
        }

        let body_val = self.lower_expr(&lambda.body, analysis)?;

        match ret_kind {
            ValueKind::Number => {
                let v = body_val.into_number()?;
                self.builder.build_return(Some(&v)).map_err(|e| e.to_string())?;
            }
            ValueKind::Bool => {
                let v = body_val.into_bool()?;
                self.builder.build_return(Some(&v)).map_err(|e| e.to_string())?;
            }
            ValueKind::String => {
                let v = body_val.into_string()?;
                self.builder.build_return(Some(&v)).map_err(|e| e.to_string())?;
            }
            ValueKind::Object => {
                let v = body_val.into_object()?;
                self.builder.build_return(Some(&v)).map_err(|e| e.to_string())?;
            }
            ValueKind::Vector => {
                let v = body_val.into_vector()?;
                self.builder.build_return(Some(&v)).map_err(|e| e.to_string())?;
            }
        }

        self.exit_scope();

        // ── Restaura el estado del builder ───────────────────────────────────
        self.current_type = saved_type;
        self.current_method = saved_method;
        if let Some(block) = saved_block {
            self.builder.position_at_end(block);
        }

        // ── Devuelve el puntero a la función casteado a i8* (Object) ─────────
        // Esto permite almacenar la lambda en variables, pasarla como argumento
        // o construir manualmente un wrapper de functor.
        let i8ptr = self.context.i8_type().ptr_type(AddressSpace::default());
        let fn_ptr = function.as_global_value().as_pointer_value();
        let casted = self
            .builder
            .build_pointer_cast(fn_ptr, i8ptr, &format!("{}_ptr", fn_name))
            .map_err(|e| e.to_string())?;

        Ok(CodegenValue::Object(casted))
    }
}
