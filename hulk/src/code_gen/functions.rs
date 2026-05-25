use crate::ast::{FunctionDecl, Item, Program, TypeDecl};
use crate::semantic::types::SemanticType;
use crate::semantic::SemanticAnalysis;

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

    pub(super) fn declare_methods(
        &mut self,
        program: &Program,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
        for item in &program.items {
            let Item::Type(typ) = item else {
                continue;
            };

            for method in &typ.methods {
                let (params, ret) = self.method_signature(typ, method, analysis)?;
                let fn_type = self.fn_type_for_signature(&params, ret);
                let name = self.method_symbol_name(&typ.name, &method.name);
                let function = self.module.add_function(&name, fn_type, None);

                self.functions.insert(
                    name,
                    FunctionInfo {
                        function,
                        params,
                        ret,
                    },
                );
            }
        }

        Ok(())
    }

    pub(super) fn define_functions(
        &mut self,
        program: &Program,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
        self.current_type = None;
        self.current_method = None;
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

                let kind = info.params[idx];
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
                    ValueKind::String | ValueKind::Object => {
                        self.builder
                            .build_store(ptr, arg.into_pointer_value())
                            .map_err(|e| e.to_string())?;
                    }
                }

                self.insert_var(
                    param.name.clone(),
                    VarInfo { ptr, kind },
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
                ValueKind::String => {
                    let str_val = value.into_string()?;
                    self.builder
                        .build_return(Some(&str_val))
                        .map_err(|e| e.to_string())?;
                }
                ValueKind::Object => {
                    let obj_val = value.into_object()?;
                    self.builder
                        .build_return(Some(&obj_val))
                        .map_err(|e| e.to_string())?;
                }
            }

            self.exit_scope();
        }

        Ok(())
    }

    pub(super) fn define_methods(
        &mut self,
        program: &Program,
        analysis: &SemanticAnalysis,
    ) -> Result<(), String> {
        for item in &program.items {
            let Item::Type(typ) = item else {
                continue;
            };

            for method in &typ.methods {
                self.current_type = Some(typ.name.clone());
                self.current_method = Some(method.name.clone());
                let name = self.method_symbol_name(&typ.name, &method.name);
                let info = self
                    .functions
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| format!("Funcion de metodo no declarada: {}", name))?;

                let entry = self.context.append_basic_block(info.function, "entry");
                self.builder.position_at_end(entry);
                self.enter_scope();

                let self_arg = info
                    .function
                    .get_nth_param(0)
                    .ok_or_else(|| format!("Falta self para el metodo {}", name))?;
                self_arg.set_name("self");
                let self_ptr = self.alloca_for_kind(&ValueKind::Object, "self")?;
                self.builder
                    .build_store(self_ptr, self_arg.into_pointer_value())
                    .map_err(|e| e.to_string())?;
                self.insert_var(
                    "self".to_string(),
                    VarInfo {
                        ptr: self_ptr,
                        kind: ValueKind::Object,
                    },
                );

                for (idx, param) in method.params.iter().enumerate() {
                    let arg = info
                        .function
                        .get_nth_param((idx + 1) as u32)
                        .ok_or_else(|| "Parametro faltante en metodo".to_string())?;
                    arg.set_name(&param.name);

                    let kind = info.params[idx + 1];
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
                        ValueKind::String | ValueKind::Object => {
                            self.builder
                                .build_store(ptr, arg.into_pointer_value())
                                .map_err(|e| e.to_string())?;
                        }
                    }

                    self.insert_var(
                        param.name.clone(),
                        VarInfo { ptr, kind },
                    );
                }

                let value = self.lower_expr(&method.body, analysis)?;
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
                    ValueKind::String => {
                        let str_val = value.into_string()?;
                        self.builder
                            .build_return(Some(&str_val))
                            .map_err(|e| e.to_string())?;
                    }
                    ValueKind::Object => {
                        let obj_val = value.into_object()?;
                        self.builder
                            .build_return(Some(&obj_val))
                            .map_err(|e| e.to_string())?;
                    }
                }

                self.exit_scope();
                self.current_type = None;
                self.current_method = None;
            }
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
            .map(|kind| self.basic_type_for_kind(kind).expect("tipo de parametro invalido").into())
            .collect::<Vec<_>>();

        match ret {
            ValueKind::Number => self.f64_type.fn_type(&param_types, false),
            ValueKind::Bool => self.bool_type.fn_type(&param_types, false),
            ValueKind::String | ValueKind::Object => self
                .context
                .i8_type()
                .ptr_type(inkwell::AddressSpace::default())
                .fn_type(&param_types, false),
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

    fn method_signature(
        &self,
        typ: &TypeDecl,
        method: &FunctionDecl,
        analysis: &SemanticAnalysis,
    ) -> Result<(Vec<ValueKind>, ValueKind), String> {
        let shape = analysis
            .type_shapes
            .get(&typ.name)
            .ok_or_else(|| format!("Tipo no encontrado en simbolos: {}", typ.name))?;

        let SemanticType::Function(params, ret) = shape
            .methods
            .get(&method.name)
            .ok_or_else(|| format!("Metodo no encontrado en tipo {}: {}", typ.name, method.name))?
        else {
            return Err(format!("La firma de {}.{} no es una funcion", typ.name, method.name));
        };

        let mut param_kinds = Vec::with_capacity(params.len() + 1);
        param_kinds.push(ValueKind::Object);
        param_kinds.extend(
            params
                .iter()
                .map(|typ| self.value_kind_from_semantic(typ))
                .collect::<Result<Vec<_>, _>>()?,
        );

        let ret_kind = self.value_kind_from_semantic(ret.as_ref())?;

        Ok((param_kinds, ret_kind))
    }
}
