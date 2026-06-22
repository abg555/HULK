use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::node_ids;
use crate::semantic::symbol_table::SymbolKind;
use crate::semantic::types::SemanticType;
use crate::semantic::{ProtocolShape, SemanticContext};

pub fn desugar_program(program: Program, context: &SemanticContext) -> Program {
    let mut desugar = FunctorDesugar::new(context);
    let mut items = Vec::new();

    for item in program.items {
        items.push(desugar.desugar_item(item));
    }

    let mut lowered = Program { items: Vec::new() };
    lowered.items.extend(desugar.synthetic_protocols);
    lowered.items.extend(desugar.wrapper_types);
    lowered.items.extend(items);
    node_ids::assign_program_node_ids(&mut lowered);
    lowered
}

struct FunctorDesugar<'a> {
    context: &'a SemanticContext,
    synthetic_protocols: Vec<Item>,
    wrapper_types: Vec<Item>,
    function_protocols: HashMap<String, String>,
    lowered_function_scopes: Vec<HashSet<String>>,
    next_protocol: usize,
    next_wrapper: usize,
}

impl<'a> FunctorDesugar<'a> {
    fn new(context: &'a SemanticContext) -> Self {
        Self {
            context,
            synthetic_protocols: Vec::new(),
            wrapper_types: Vec::new(),
            function_protocols: HashMap::new(),
            lowered_function_scopes: vec![HashSet::new()],
            next_protocol: 0,
            next_wrapper: 0,
        }
    }

    fn desugar_item(&mut self, item: Item) -> Item {
        match item {
            Item::Import(imp) => Item::Import(imp),
            Item::Export(exp) => Item::Export(exp),
            Item::Function(func) => Item::Function(self.desugar_function_decl(func)),
            Item::Type(mut typ) => {
                self.enter_scope();
                for param in &typ.param {
                    if param.types.as_ref().is_some_and(Self::is_function_type_ref) {
                        self.mark_lowered_function(&param.name);
                    }
                }

                typ.param = typ
                    .param
                    .into_iter()
                    .map(|param| self.desugar_param(param))
                    .collect();
                typ.parent = typ.parent.map(|parent| self.desugar_type_ref(parent));
                typ.fields = typ
                    .fields
                    .into_iter()
                    .map(|field| self.desugar_field_decl(field))
                    .collect();
                typ.methods = typ
                    .methods
                    .into_iter()
                    .map(|method| self.desugar_function_decl(method))
                    .collect();
                self.exit_scope();
                Item::Type(typ)
            }
            Item::Protocol(mut proto) => {
                proto.parent = proto
                    .parent
                    .map(|parent| Box::new(self.desugar_type_ref(*parent)));
                proto.methods = proto
                    .methods
                    .into_iter()
                    .map(|mut method| {
                        method.params = method
                            .params
                            .into_iter()
                            .map(|param| self.desugar_param(param))
                            .collect();
                        method.return_type = self.desugar_type_ref(method.return_type);
                        method
                    })
                    .collect();
                Item::Protocol(proto)
            }
            Item::Macro(mut macr) => {
                self.enter_scope();
                macr.params = macr
                    .params
                    .into_iter()
                    .map(|mut param| {
                        param.type_info = param
                            .type_info
                            .map(|type_ref| self.desugar_type_ref(type_ref));
                        param
                    })
                    .collect();
                macr.body = self.desugar_expr(macr.body);
                self.exit_scope();
                Item::Macro(macr)
            }
            Item::GlobalExpr(expr) => Item::GlobalExpr(self.desugar_expr(expr)),
        }
    }

    fn desugar_function_decl(&mut self, mut func: FunctionDecl) -> FunctionDecl {
        self.enter_scope();
        for param in &func.params {
            if param.types.as_ref().is_some_and(Self::is_function_type_ref) {
                self.mark_lowered_function(&param.name);
            }
        }

        func.params = func
            .params
            .into_iter()
            .map(|param| self.desugar_param(param))
            .collect();
        func.return_type = func
            .return_type
            .map(|return_type| self.desugar_type_ref(return_type));
        func.body = self.desugar_expr(func.body);
        self.exit_scope();
        func
    }

    fn desugar_param(&mut self, mut param: Param) -> Param {
        param.types = param.types.map(|type_ref| self.desugar_type_ref(type_ref));
        param
    }

    fn desugar_field_decl(&mut self, mut field: FieldDecl) -> FieldDecl {
        let expected = field
            .type_annotation
            .as_ref()
            .map(SemanticType::from_type_ref);
        field.type_annotation = field
            .type_annotation
            .map(|type_ref| self.desugar_type_ref(type_ref));
        field.initializer = self.desugar_expr_for_expected(field.initializer, expected.as_ref());
        field
    }

    fn desugar_expr(&mut self, expr: Expr) -> Expr {
        let original = expr.clone();
        match expr.kind {
            KindExpr::Literal(_) | KindExpr::Variable(_) => original,
            KindExpr::Binary(mut bin) => {
                bin.left = Box::new(self.desugar_expr(*bin.left));
                bin.right = Box::new(self.desugar_expr(*bin.right));
                self.rebuild_expr(original, KindExpr::Binary(bin))
            }
            KindExpr::Unary(mut unary) => {
                unary.right = Box::new(self.desugar_expr(*unary.right));
                self.rebuild_expr(original, KindExpr::Unary(unary))
            }
            KindExpr::Call(call) => self.desugar_call_expr(original, call),
            KindExpr::BaseCall(mut call) => {
                call.arguments = call
                    .arguments
                    .into_iter()
                    .map(|arg| self.desugar_expr(arg))
                    .collect();
                self.rebuild_expr(original, KindExpr::BaseCall(call))
            }
            KindExpr::MacroCall(mut call) => {
                call.arguments = call
                    .arguments
                    .into_iter()
                    .map(|mut arg| {
                        arg.value = self.desugar_expr(arg.value);
                        arg
                    })
                    .collect();
                call.action = call
                    .action
                    .map(|action| Box::new(self.desugar_expr(*action)));
                self.rebuild_expr(original, KindExpr::MacroCall(call))
            }
            KindExpr::Let(mut let_expr) => {
                self.enter_scope();
                let mut lowered_names = Vec::new();
                let_expr.bindings = let_expr
                    .bindings
                    .into_iter()
                    .map(|mut binding| {
                        let expected = binding.types.as_ref().map(SemanticType::from_type_ref);
                        let lowers_function = binding
                            .types
                            .as_ref()
                            .is_some_and(Self::is_function_type_ref);
                        binding.types = binding
                            .types
                            .map(|type_ref| self.desugar_type_ref(type_ref));
                        binding.initializer =
                            self.desugar_expr_for_expected(binding.initializer, expected.as_ref());
                        if lowers_function {
                            lowered_names.push(binding.name.clone());
                        }
                        binding
                    })
                    .collect();
                for name in lowered_names {
                    self.mark_lowered_function(&name);
                }
                let_expr.body = Box::new(self.desugar_expr(*let_expr.body));
                self.exit_scope();
                self.rebuild_expr(original, KindExpr::Let(let_expr))
            }
            KindExpr::Block(mut block) => {
                self.enter_scope();
                block.expressions = block
                    .expressions
                    .into_iter()
                    .map(|sub| self.desugar_expr(sub))
                    .collect();
                self.exit_scope();
                self.rebuild_expr(original, KindExpr::Block(block))
            }
            KindExpr::If(mut if_expr) => {
                if_expr.condition = Box::new(self.desugar_expr(*if_expr.condition));
                if_expr.then_branch = Box::new(self.desugar_expr(*if_expr.then_branch));
                if_expr.elif_branches = if_expr
                    .elif_branches
                    .into_iter()
                    .map(|(cond, body)| (self.desugar_expr(cond), self.desugar_expr(body)))
                    .collect();
                if_expr.else_branch = Box::new(self.desugar_expr(*if_expr.else_branch));
                self.rebuild_expr(original, KindExpr::If(if_expr))
            }
            KindExpr::While(mut while_expr) => {
                while_expr.condition = Box::new(self.desugar_expr(*while_expr.condition));
                while_expr.body = Box::new(self.desugar_expr(*while_expr.body));
                self.rebuild_expr(original, KindExpr::While(while_expr))
            }
            KindExpr::For(mut for_expr) => {
                for_expr.iterable = Box::new(self.desugar_expr(*for_expr.iterable));
                self.enter_scope();
                for_expr.body = Box::new(self.desugar_expr(*for_expr.body));
                self.exit_scope();
                self.rebuild_expr(original, KindExpr::For(for_expr))
            }
            KindExpr::Assign(mut assign) => {
                let expected = self.context.inferred_types.get(&assign.target.id).cloned();
                assign.target = Box::new(self.desugar_expr(*assign.target));
                assign.value =
                    Box::new(self.desugar_expr_for_expected(*assign.value, expected.as_ref()));
                self.rebuild_expr(original, KindExpr::Assign(assign))
            }
            KindExpr::MemberAccess(mut member) => {
                member.object = Box::new(self.desugar_expr(*member.object));
                self.rebuild_expr(original, KindExpr::MemberAccess(member))
            }
            KindExpr::Index(mut index) => {
                index.object = Box::new(self.desugar_expr(*index.object));
                index.index = Box::new(self.desugar_expr(*index.index));
                self.rebuild_expr(original, KindExpr::Index(index))
            }
            KindExpr::Array(mut array) => {
                array.elements = array
                    .elements
                    .into_iter()
                    .map(|element| self.desugar_expr(element))
                    .collect();
                self.rebuild_expr(original, KindExpr::Array(array))
            }
            KindExpr::ArrayComprehension(mut comp) => {
                comp.iterable = Box::new(self.desugar_expr(*comp.iterable));
                self.enter_scope();
                comp.element = Box::new(self.desugar_expr(*comp.element));
                self.exit_scope();
                self.rebuild_expr(original, KindExpr::ArrayComprehension(comp))
            }
            KindExpr::Lambda(mut lambda) => {
                self.enter_scope();
                lambda.params = lambda
                    .params
                    .into_iter()
                    .map(|param| self.desugar_param(param))
                    .collect();
                lambda.return_type = lambda
                    .return_type
                    .map(|return_type| self.desugar_type_ref(return_type));
                lambda.body = Box::new(self.desugar_expr(*lambda.body));
                self.exit_scope();
                self.rebuild_expr(original, KindExpr::Lambda(lambda))
            }
            KindExpr::New(mut new_expr) => {
                // If the new expression targets a named type, try to get constructor shapes
                let expected_args = match &new_expr.type_info {
                    TypeRef::Custom(name) => self
                        .context
                        .type_shapes
                        .get(name)
                        .map(|shape| shape.ctor_params.clone())
                        .unwrap_or_default(),
                    _ => vec![],
                };
                new_expr.arguments = new_expr
                    .arguments
                    .into_iter()
                    .enumerate()
                    .map(|(idx, arg)| self.desugar_expr_for_expected(arg, expected_args.get(idx)))
                    .collect();
                self.rebuild_expr(original, KindExpr::New(new_expr))
            }
            KindExpr::Is(mut is_expr) => {
                is_expr.expression = Box::new(self.desugar_expr(*is_expr.expression));
                is_expr.type_info = self.desugar_type_ref(is_expr.type_info);
                self.rebuild_expr(original, KindExpr::Is(is_expr))
            }
            KindExpr::As(mut as_expr) => {
                as_expr.expression = Box::new(self.desugar_expr(*as_expr.expression));
                as_expr.type_info = self.desugar_type_ref(as_expr.type_info);
                self.rebuild_expr(original, KindExpr::As(as_expr))
            }
            KindExpr::Match(mut match_expr) => {
                match_expr.expression = Box::new(self.desugar_expr(*match_expr.expression));
                match_expr.cases = match_expr
                    .cases
                    .into_iter()
                    .map(|mut case| {
                        case.pattern = self.desugar_pattern(case.pattern);
                        case.body = self.desugar_expr(case.body);
                        case
                    })
                    .collect();
                self.rebuild_expr(original, KindExpr::Match(match_expr))
            }
        }
    }

    fn desugar_call_expr(&mut self, expr: Expr, mut call: CallExpr) -> Expr {
        let original_callee = (*call.callee).clone();
        let constructor_name = self.constructor_call_name(&original_callee);
        let expected_args = self.expected_call_args(&call);

        call.callee = Box::new(self.desugar_expr(*call.callee));
        call.arguments = call
            .arguments
            .into_iter()
            .enumerate()
            .map(|(idx, arg)| self.desugar_expr_for_expected(arg, expected_args.get(idx)))
            .collect();

        if let Some(type_name) = constructor_name {
            return self.rebuild_expr(
                expr,
                KindExpr::New(NewExpr {
                    type_info: TypeRef::Custom(type_name),
                    arguments: call.arguments,
                    initializer: None,
                }),
            );
        }

        let should_invoke = self.should_lower_call_to_invoke(&original_callee);
        if should_invoke {
            call.callee = Box::new(self.member_access_expr(*call.callee, "invoke"));
        }

        self.rebuild_expr(expr, KindExpr::Call(call))
    }

    fn desugar_expr_for_expected(&mut self, expr: Expr, expected: Option<&SemanticType>) -> Expr {
        let actual = self.context.inferred_types.get(&expr.id).cloned();
        let expr = self.desugar_expr(expr);

        let Some(expected) = expected else {
            return expr;
        };
        let Some(actual) = actual else {
            return expr;
        };

        match (expected, &actual) {
            (SemanticType::Custom(protocol_name), SemanticType::Function(_, _)) => self
                .lookup_functor_invoke_type(protocol_name)
                .map_or(expr.clone(), |invoke_signature| {
                    self.wrap_function_as_functor(expr, &invoke_signature)
                }),
            (SemanticType::Function(_, _), SemanticType::Function(_, _)) => {
                self.wrap_function_as_functor(expr, expected)
            }
            _ => expr,
        }
    }

    fn expected_call_args(&self, call: &CallExpr) -> Vec<SemanticType> {
        if let Some(type_name) = self.constructor_call_name(&call.callee) {
            return self
                .context
                .type_shapes
                .get(&type_name)
                .map(|shape| shape.ctor_params.clone())
                .unwrap_or_default();
        }

        let Some(callee_ty) = self.context.inferred_types.get(&call.callee.id) else {
            return Vec::new();
        };

        match callee_ty {
            SemanticType::Function(params, _) => params.clone(),
            SemanticType::Custom(type_name) => {
                if let Some(SemanticType::Function(params, _)) =
                    self.lookup_functor_invoke_type(type_name)
                {
                    params
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    fn should_lower_call_to_invoke(&self, callee: &Expr) -> bool {
        let Some(callee_ty) = self.context.inferred_types.get(&callee.id) else {
            return false;
        };

        match callee_ty {
            SemanticType::Custom(type_name) => self.lookup_functor_invoke_type(type_name).is_some(),
            SemanticType::Function(_, _) => self.function_callee_was_lowered(callee),
            _ => false,
        }
    }

    fn function_callee_was_lowered(&self, callee: &Expr) -> bool {
        match &callee.kind {
            KindExpr::Variable(var) => {
                self.is_lowered_function(&var.name)
                    && !self
                        .context
                        .global_symbols
                        .get(&var.name)
                        .is_some_and(|symbol| symbol.kind == SymbolKind::Function)
            }
            // Regular OO method calls are also represented as MemberAccess, and
            // rewriting all of them to `.invoke(...)` breaks dispatch (e.g. p.getX()).
            // Member accesses should only be lowered through the semantic-type
            // branch in `should_lower_call_to_invoke` when they are functor-like
            // custom values, not just because they are member accesses.
            KindExpr::MemberAccess(_) => false,
            _ => false,
        }
    }

    fn constructor_call_name(&self, callee: &Expr) -> Option<String> {
        let KindExpr::Variable(var) = &callee.kind else {
            return None;
        };

        if self.context.inferred_types.contains_key(&callee.id) {
            return None;
        }

        self.context
            .global_symbols
            .get(&var.name)
            .filter(|symbol| symbol.kind == SymbolKind::Type)
            .map(|_| var.name.clone())
    }

    fn desugar_pattern(&mut self, pattern: Pattern) -> Pattern {
        match pattern {
            Pattern::Identifier {
                name,
                type_restriction,
            } => Pattern::Identifier {
                name,
                type_restriction: type_restriction.map(|type_ref| self.desugar_type_ref(type_ref)),
            },
            Pattern::Binary {
                left,
                operator,
                right,
            } => Pattern::Binary {
                left: Box::new(self.desugar_pattern(*left)),
                operator,
                right: Box::new(self.desugar_pattern(*right)),
            },
            Pattern::Unary { operator, operand } => Pattern::Unary {
                operator,
                operand: Box::new(self.desugar_pattern(*operand)),
            },
            Pattern::Literal(_) | Pattern::Default => pattern,
        }
    }

    fn desugar_type_ref(&mut self, type_ref: TypeRef) -> TypeRef {
        match type_ref {
            TypeRef::Vector(inner) => TypeRef::Vector(Box::new(self.desugar_type_ref(*inner))),
            TypeRef::Function(params, ret) => {
                let params = params
                    .into_iter()
                    .map(|param| self.desugar_type_ref(param))
                    .collect::<Vec<_>>();
                let ret = self.desugar_type_ref(*ret);
                let protocol = self.ensure_functor_protocol_for_type_refs(&params, &ret);
                TypeRef::Custom(protocol)
            }
            other => other,
        }
    }

    fn ensure_functor_protocol_for_type_refs(
        &mut self,
        params: &[TypeRef],
        ret: &TypeRef,
    ) -> String {
        let key = format!(
            "({}) -> {}",
            params
                .iter()
                .map(Self::type_ref_key)
                .collect::<Vec<_>>()
                .join(","),
            Self::type_ref_key(ret)
        );

        if let Some(name) = self.function_protocols.get(&key) {
            return name.clone();
        }

        let name = self.fresh_name("_Functor");
        let method_params = params
            .iter()
            .enumerate()
            .map(|(idx, type_ref)| Param {
                name: format!("_arg{}", idx),
                types: Some(type_ref.clone()),
                is_variadic: false,
            })
            .collect();
        self.synthetic_protocols.push(Item::Protocol(ProtocolDecl {
            name: name.clone(),
            parent: None,
            methods: vec![ProtocolMethod {
                name: "invoke".to_string(),
                params: method_params,
                return_type: ret.clone(),
            }],
        }));
        self.function_protocols.insert(key, name.clone());
        name
    }

    fn wrap_function_as_functor(&mut self, expr: Expr, invoke_signature: &SemanticType) -> Expr {
        let SemanticType::Function(params, ret) = invoke_signature else {
            return expr;
        };

        let wrapper_name = self.fresh_name("_FunctorWrapper");
        let fn_type = SemanticType::Function(params.clone(), ret.clone());
        let fn_type_ref = Self::semantic_type_to_type_ref(&fn_type);
        let ret_type_ref = Self::semantic_type_to_type_ref(ret);
        let method_params = params
            .iter()
            .enumerate()
            .map(|(idx, param_ty)| Param {
                name: format!("_arg{}", idx),
                types: Self::semantic_type_to_type_ref(param_ty),
                is_variadic: false,
            })
            .collect::<Vec<_>>();
        let call_args = method_params
            .iter()
            .map(|param| self.variable_expr(&param.name))
            .collect::<Vec<_>>();

        let wrapper = TypeDecl {
            name: wrapper_name.clone(),
            param: vec![Param {
                name: "__fn_arg".to_string(),
                types: fn_type_ref.clone(),
                is_variadic: false,
            }],
            parent: None,
            parent_arg: Some(Vec::new()),
            fields: vec![FieldDecl {
                name: "__fn".to_string(),
                type_annotation: fn_type_ref,
                initializer: self.variable_expr("__fn_arg"),
            }],
            methods: vec![FunctionDecl {
                name: "invoke".to_string(),
                params: method_params,
                return_type: ret_type_ref,
                body: self.call_expr(self.member_access_expr(self.self_expr(), "__fn"), call_args),
            }],
        };
        self.wrapper_types.push(Item::Type(wrapper));

        let mut lowered = mk_expr(KindExpr::New(NewExpr {
            type_info: TypeRef::Custom(wrapper_name),
            arguments: vec![expr],
            initializer: None,
        }));
        lowered.span = Span { start: 0, end: 0 };
        lowered
    }

    fn lookup_functor_invoke_type(&self, type_name: &str) -> Option<SemanticType> {
        if self.context.protocol_shapes.contains_key(type_name) {
            return self.lookup_protocol_member_type(type_name, "invoke");
        }

        if let Some(shape) = self.context.type_shapes.get(type_name) {
            if let Some(signature) = shape.methods.get("invoke") {
                return Some(signature.clone());
            }
        }

        None
    }

    fn lookup_protocol_member_type(
        &self,
        protocol_name: &str,
        member: &str,
    ) -> Option<SemanticType> {
        let mut current = Some(protocol_name.to_string());
        let mut visited = HashSet::new();

        while let Some(name) = current {
            if !visited.insert(name.clone()) {
                break;
            }

            let ProtocolShape { methods, parent } = self.context.protocol_shapes.get(&name)?;
            if let Some(signature) = methods.get(member) {
                return Some(signature.clone());
            }

            current = parent.clone();
        }

        None
    }

    fn semantic_type_to_type_ref(ty: &SemanticType) -> Option<TypeRef> {
        match ty {
            SemanticType::Number => Some(TypeRef::Number),
            SemanticType::String => Some(TypeRef::String),
            SemanticType::Boolean => Some(TypeRef::Boolean),
            SemanticType::Vector(inner) => {
                Self::semantic_type_to_type_ref(inner).map(|inner| TypeRef::Vector(Box::new(inner)))
            }
            SemanticType::Function(params, ret) => {
                let mut param_refs = Vec::new();
                for param in params {
                    param_refs.push(Self::semantic_type_to_type_ref(param)?);
                }
                Some(TypeRef::Function(
                    param_refs,
                    Box::new(Self::semantic_type_to_type_ref(ret)?),
                ))
            }
            SemanticType::Custom(name) => Some(TypeRef::Custom(name.clone())),
            SemanticType::Unknown => None,
        }
    }

    fn is_function_type_ref(type_ref: &TypeRef) -> bool {
        matches!(type_ref, TypeRef::Function(_, _))
    }

    fn type_ref_key(type_ref: &TypeRef) -> String {
        match type_ref {
            TypeRef::Number => "Number".to_string(),
            TypeRef::String => "String".to_string(),
            TypeRef::Boolean => "Boolean".to_string(),
            TypeRef::Vector(inner) => format!("{}[]", Self::type_ref_key(inner)),
            TypeRef::Function(params, ret) => format!(
                "({}) -> {}",
                params
                    .iter()
                    .map(Self::type_ref_key)
                    .collect::<Vec<_>>()
                    .join(","),
                Self::type_ref_key(ret)
            ),
            TypeRef::Custom(name) => name.clone(),
        }
    }

    fn fresh_name(&mut self, prefix: &str) -> String {
        loop {
            let idx = if prefix == "_Functor" {
                let idx = self.next_protocol;
                self.next_protocol += 1;
                idx
            } else {
                let idx = self.next_wrapper;
                self.next_wrapper += 1;
                idx
            };
            let name = format!("{}{}", prefix, idx);
            if !self.context.global_symbols.contains_key(&name)
                && !self
                    .synthetic_protocols
                    .iter()
                    .any(|item| matches!(item, Item::Protocol(proto) if proto.name == name))
                && !self
                    .wrapper_types
                    .iter()
                    .any(|item| matches!(item, Item::Type(typ) if typ.name == name))
            {
                return name;
            }
        }
    }

    fn enter_scope(&mut self) {
        self.lowered_function_scopes.push(HashSet::new());
    }

    fn exit_scope(&mut self) {
        self.lowered_function_scopes.pop();
    }

    fn mark_lowered_function(&mut self, name: &str) {
        if let Some(scope) = self.lowered_function_scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn is_lowered_function(&self, name: &str) -> bool {
        self.lowered_function_scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(name))
    }

    fn rebuild_expr(&self, original: Expr, kind: KindExpr) -> Expr {
        Expr {
            id: original.id,
            span: original.span,
            kind,
        }
    }

    fn variable_expr(&self, name: &str) -> Expr {
        mk_expr(KindExpr::Variable(VariableExpr {
            name: name.to_string(),
        }))
    }

    fn self_expr(&self) -> Expr {
        self.variable_expr("self")
    }

    fn member_access_expr(&self, object: Expr, field: &str) -> Expr {
        mk_expr(KindExpr::MemberAccess(MemberAccessExpr {
            object: Box::new(object),
            field: field.to_string(),
        }))
    }

    fn call_expr(&self, callee: Expr, arguments: Vec<Expr>) -> Expr {
        mk_expr(KindExpr::Call(CallExpr {
            callee: Box::new(callee),
            arguments,
        }))
    }
}
