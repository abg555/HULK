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

            let (params, ret) = self.function_signature(func, analysis)?;
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

    fn function_signature(
        &self,
        func: &FunctionDecl,
        analysis: &SemanticAnalysis,
    ) -> Result<(Vec<ValueKind>, ValueKind), String> {
        let symbol = analysis
            .global_symbols
            .get(&func.name)
            .ok_or_else(|| format!("Funcion no encontrada en simbolos: {}", func.name))?;

        let SemanticType::Function(params, ret) = &symbol.typ else {
            return Err(format!("Simbolo {} no es funcion", func.name));
        };

        let param_kinds = params
            .iter()
            .map(|typ| self.value_kind_from_semantic(typ))
            .collect::<Result<Vec<_>, _>>()?;
        let ret_kind = self.value_kind_from_semantic(ret.as_ref())?;

        Ok((param_kinds, ret_kind))
    }
}
