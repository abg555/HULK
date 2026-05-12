use crate::ast::{FunctionDecl, Item, Program};
use crate::semantic::SemanticAnalysis;
use crate::semantic::types::SemanticType;

use super::{CodeGenerator, FunctionInfo, ValueKind, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn declare_functions(
        &mut self,
        program: &Program,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
        for item in &program.items {
            let Item::Function(func) = item else {
                continue;
            };

            let params = self.function_param_kinds(func, analysis)?;
            let ret = self.function_return_kind(func, analysis)?;
            let fn_type = self.fn_type_for_signature(&params, ret);
            let function = self.module.add_function(&func.name, fn_type, None);

            self.functions.insert(
                func.name.clone(),
                FunctionInfo {
                    function,
                    params,
                    ret,
                },
            );
        }

        Ok(())
    }

    pub(super) fn define_functions(
        &mut self,
        program: &Program,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
        for item in &program.items {
            let Item::Function(func) = item else {
                continue;
            };

            let info = self
                .functions
                .get(&func.name)
                .cloned()
                .ok_or_else(|| format!("Funcion no declarada: {}", func.name))?;

            let entry = self.context.append_basic_block(info.function, "entry");
            self.builder.position_at_end(entry);
            self.enter_scope();

            for (idx, param) in func.params.iter().enumerate() {
                let arg = info
                    .function
                    .get_nth_param(idx as u32)
                    .ok_or_else(|| "Parametro faltante".to_string())?;
                arg.set_name(&param.name);

                let (ptr, kind) = match info.params[idx] {
                    ValueKind::Number => {
                        let ptr = self
                            .builder
                            .build_alloca(self.f64_type, &param.name)
                            .map_err(|e| e.to_string())?;
                        self.builder
                            .build_store(ptr, arg.into_float_value())
                            .map_err(|e| e.to_string())?;
                        (ptr, ValueKind::Number)
                    }
                    ValueKind::Bool => {
                        let ptr = self
                            .builder
                            .build_alloca(self.bool_type, &param.name)
                            .map_err(|e| e.to_string())?;
                        self.builder
                            .build_store(ptr, arg.into_int_value())
                            .map_err(|e| e.to_string())?;
                        (ptr, ValueKind::Bool)
                    }
                };

                self.insert_var(
                    param.name.clone(),
                    VarInfo {
                        ptr,
                        kind,
                    },
                );
            }

            let value = self.lower_expr(&func.body, analysis)?;
            match info.ret {
                ValueKind::Number => {
                    let number = value.into_number()?;
                    self.builder
                        .build_return(Some(&number))
                        .map_err(|e| e.to_string())?;
                }
                ValueKind::Bool => {
                    let boolean = value.into_bool()?;
                    self.builder
                        .build_return(Some(&boolean))
                        .map_err(|e| e.to_string())?;
                }
            }

            self.exit_scope();
        }

        Ok(())
    }

    pub(super) fn get_function(&self, name: &str) -> Option<&FunctionInfo<'ctx>> {
        self.functions.get(name)
    }

    fn fn_type_for_signature(
        &self,
        params: &[ValueKind],
        ret: ValueKind,
    ) -> inkwell::types::FunctionType<'ctx> {
        let param_types = params
            .iter()
            .map(|kind| match kind {
                ValueKind::Number => self.f64_type.into(),
                ValueKind::Bool => self.bool_type.into(),
            })
            .collect::<Vec<_>>();

        match ret {
            ValueKind::Number => self.f64_type.fn_type(&param_types, false),
            ValueKind::Bool => self.bool_type.fn_type(&param_types, false),
        }
    }

    fn function_param_kinds(
        &self,
        func: &FunctionDecl,
        analysis: &SemanticAnalysis,
    ) -> Result<Vec<ValueKind>, String> {
        func.params
            .iter()
            .map(|param| {
                if let Some(type_ref) = &param.types {
                    let semantic = SemanticType::from_type_ref(type_ref);
                    self.value_kind_from_semantic(&semantic)
                } else if let Some(map) = analysis.inferred_function_params.get(&func.name)
                    && let Some(semantic) = map.get(&param.name)
                {
                    self.value_kind_from_semantic(semantic)
                } else {
                    Err(format!(
                        "Parametro sin tipo en funcion {}: {}",
                        func.name, param.name
                    ))
                }
            })
            .collect()
    }

    fn function_return_kind(
        &self,
        func: &FunctionDecl,
        analysis: &SemanticAnalysis,
    ) -> Result<ValueKind, String> {
        if let Some(type_ref) = &func.return_type {
            let semantic = SemanticType::from_type_ref(type_ref);
            return self.value_kind_from_semantic(&semantic);
        }

        if let Some(semantic) = analysis.inferred_function_returns.get(&func.name) {
            return self.value_kind_from_semantic(semantic);
        }

        if let Some(semantic) = analysis.inferred_types.get(&func.body.id) {
            return self.value_kind_from_semantic(semantic);
        }

        Err(format!("No se pudo inferir retorno de {}", func.name))
    }
}
