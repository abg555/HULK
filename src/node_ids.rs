use crate::ast::*;

pub fn assign_program_node_ids(program: &mut Program) {
    let mut next_id = 1_u32;

    for item in &mut program.items {
        assign_item_node_ids(item, &mut next_id);
    }
}

fn assign_item_node_ids(item: &mut Item, next_id: &mut u32) {
    match item {
        Item::Import(_) | Item::Export(_) => {}
        Item::Function(func) => assign_expr_node_ids(&mut func.body, next_id),
        Item::Type(typ) => {
            for field in &mut typ.fields {
                assign_expr_node_ids(&mut field.initializer, next_id);
            }
            for method in &mut typ.methods {
                assign_expr_node_ids(&mut method.body, next_id);
            }
        }
        Item::Protocol(_) => {}
        Item::Macro(macr) => assign_expr_node_ids(&mut macr.body, next_id),
        Item::GlobalExpr(expr) => assign_expr_node_ids(expr, next_id),
    }
}

fn assign_expr_node_ids(expr: &mut Expr, next_id: &mut u32) {
    expr.id = NodeId(*next_id);
    *next_id = next_id.saturating_add(1);

    match &mut expr.kind {
        KindExpr::Literal(_) | KindExpr::Variable(_) => {}
        KindExpr::Binary(bin) => {
            assign_expr_node_ids(&mut bin.left, next_id);
            assign_expr_node_ids(&mut bin.right, next_id);
        }
        KindExpr::Unary(unary) => assign_expr_node_ids(&mut unary.right, next_id),
        KindExpr::Call(call) => {
            assign_expr_node_ids(&mut call.callee, next_id);
            for arg in &mut call.arguments {
                assign_expr_node_ids(arg, next_id);
            }
        }
        KindExpr::BaseCall(call) => {
            for arg in &mut call.arguments {
                assign_expr_node_ids(arg, next_id);
            }
        }
        KindExpr::MacroCall(call) => {
            for arg in &mut call.arguments {
                assign_expr_node_ids(&mut arg.value, next_id);
            }
            if let Some(action) = &mut call.action {
                assign_expr_node_ids(action, next_id);
            }
        }
        KindExpr::Let(let_expr) => {
            for binding in &mut let_expr.bindings {
                assign_expr_node_ids(&mut binding.initializer, next_id);
            }
            assign_expr_node_ids(&mut let_expr.body, next_id);
        }
        KindExpr::Block(block) => {
            for sub in &mut block.expressions {
                assign_expr_node_ids(sub, next_id);
            }
        }
        KindExpr::If(if_expr) => {
            assign_expr_node_ids(&mut if_expr.condition, next_id);
            assign_expr_node_ids(&mut if_expr.then_branch, next_id);
            for (cond, body) in &mut if_expr.elif_branches {
                assign_expr_node_ids(cond, next_id);
                assign_expr_node_ids(body, next_id);
            }
            assign_expr_node_ids(&mut if_expr.else_branch, next_id);
        }
        KindExpr::While(while_expr) => {
            assign_expr_node_ids(&mut while_expr.condition, next_id);
            assign_expr_node_ids(&mut while_expr.body, next_id);
        }
        KindExpr::For(for_expr) => {
            assign_expr_node_ids(&mut for_expr.iterable, next_id);
            assign_expr_node_ids(&mut for_expr.body, next_id);
        }
        KindExpr::Assign(assign) => {
            assign_expr_node_ids(&mut assign.target, next_id);
            assign_expr_node_ids(&mut assign.value, next_id);
        }
        KindExpr::MemberAccess(member) => assign_expr_node_ids(&mut member.object, next_id),
        KindExpr::Index(index) => {
            assign_expr_node_ids(&mut index.object, next_id);
            assign_expr_node_ids(&mut index.index, next_id);
        }
        KindExpr::Array(array) => {
            for elem in &mut array.elements {
                assign_expr_node_ids(elem, next_id);
            }
        }
        KindExpr::ArrayComprehension(comp) => {
            assign_expr_node_ids(&mut comp.element, next_id);
            assign_expr_node_ids(&mut comp.iterable, next_id);
        }
        KindExpr::Lambda(lambda) => assign_expr_node_ids(&mut lambda.body, next_id),
        KindExpr::New(new_expr) => {
            for arg in &mut new_expr.arguments {
                assign_expr_node_ids(arg, next_id);
            }
        }
        KindExpr::Is(is_expr) => assign_expr_node_ids(&mut is_expr.expression, next_id),
        KindExpr::As(as_expr) => assign_expr_node_ids(&mut as_expr.expression, next_id),
        KindExpr::Match(match_expr) => {
            assign_expr_node_ids(&mut match_expr.expression, next_id);
            for case in &mut match_expr.cases {
                assign_expr_node_ids(&mut case.body, next_id);
            }
        }
    }
}
