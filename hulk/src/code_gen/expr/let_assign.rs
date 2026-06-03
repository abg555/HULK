use crate::ast::{AssignExpr, LetExpr, KindExpr, MemberAccessExpr};
use crate::semantic::SemanticAnalysis;
use inkwell::AddressSpace;

use super::super::{CodegenValue, CodeGenerator, VarInfo};

impl<'ctx> CodeGenerator<'ctx> {
    pub(super) fn lower_let(
        &mut self,
        let_expr: &LetExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        self.enter_scope();

        for binding in &let_expr.bindings {
            let value = self.lower_expr(&binding.initializer, analysis)?;
            let kind = value.kind();
            let ptr = self.alloca_for_kind(&kind, &binding.name)?;
            self.store_value(ptr, value)?;

            self.insert_var(
                binding.name.clone(),
                VarInfo {
                    ptr,
                    kind,
                },
            );
        }

        let result = self.lower_expr(&let_expr.body, analysis);
        self.exit_scope();
        result
    }

    pub(super) fn lower_assign(
        &mut self,
        assign: &AssignExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        match &assign.target.kind {
            KindExpr::Variable(variable) => {
                let info = self
                    .lookup_var(&variable.name)
                    .ok_or_else(|| format!("Variable no definida: {}", variable.name))?;
                let value = self.lower_expr(&assign.value, analysis)?;

                if value.kind() != info.kind {
                    return Err(format!(
                        "Tipo incompatible en asignacion a {}",
                        variable.name
                    ));
                }

                self.store_value(info.ptr, value)?;
                self.load_value(&info.kind, info.ptr, &format!("reload_{}", variable.name))
            }
            KindExpr::MemberAccess(member) => self.lower_member_assign(member, assign, analysis),
            _ => Err("Asignacion solo soporta variables y miembros".to_string()),
        }
    }

    fn lower_member_assign(
        &mut self,
        member: &MemberAccessExpr,
        assign: &AssignExpr,
        analysis: &SemanticAnalysis,
    ) -> Result<CodegenValue<'ctx>, String> {
        let object_type = self.member_object_type(member, analysis)?;
        let object_struct = self.object_struct_type(&object_type)?;
        let object_value = self.lower_expr(&member.object, analysis)?.into_object()?;
        let typed_ptr = self
            .builder
            .build_pointer_cast(
                object_value,
                object_struct.ptr_type(AddressSpace::default()),
                "obj_cast",
            )
            .map_err(|e| e.to_string())?;

        let field_type =
            self.object_field_semantic_type(&object_type, &member.field, analysis)?;
        let field_kind = self.value_kind_from_semantic(&field_type)?;

        let value = self.lower_expr(&assign.value, analysis)?;
        if value.kind() != field_kind {
            return Err(format!(
                "Tipo incompatible en asignacion a {}",
                member.field
            ));
        }

        let field_index = self
            .object_field_names(&object_type)?
            .iter()
            .position(|name| name == &member.field)
            .ok_or_else(|| {
                format!(
                    "El tipo {} no define el campo {}",
                    object_type, member.field
                )
            })?
            + 1;

        let field_ptr = self
            .builder
            .build_struct_gep(
                object_struct,
                typed_ptr,
                field_index as u32,
                &format!("{}_field", member.field),
            )
            .map_err(|e| e.to_string())?;

        self.store_value(field_ptr, value)?;
        self.load_value(&field_kind, field_ptr, &format!("reload_{}", member.field))
    }
}
