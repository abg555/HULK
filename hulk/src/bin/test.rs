use hulk::{ProgramParser, lex_safe};
use std::fs;
use std::io::Read;

fn main() -> std::io::Result<()> {
    // Leer el archivo test.hulk
    let mut file = fs::File::open("test.hulk")?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;

    println!("{}", "=".repeat(80));
    println!("PARSER TEST - test.hulk");
    println!("{}", "=".repeat(80));
    println!();

    let mut examples = Vec::new();
    let mut current_title = String::new();
    let mut current_code = String::new();

    for line in input.lines() {
        let trimmed = line.trim_start();
        let lowered = trimmed.to_lowercase();
        let is_example_header =
            lowered.starts_with("// example") || lowered.starts_with("//ejemplo");

        if is_example_header {
            if !current_code.is_empty() {
                examples.push((current_title.clone(), current_code.clone()));
                current_code.clear();
            }
            current_title = line.to_string();
        } else if !line.trim().is_empty()
            && !line.trim().starts_with("//")
            && !line.trim().starts_with("=")
        {
            if !current_title.is_empty() {
                current_code.push_str(line);
                current_code.push('\n');
            }
        }
    }

    if !current_code.is_empty() {
        examples.push((current_title, current_code));
    }

    println!("Total de ejemplos encontrados: {}\n", examples.len());

    let parser = ProgramParser::new();
    let mut valid_count = 0;
    let mut error_count = 0;

    for (idx, (title, code)) in examples.iter().enumerate() {
        print!("[{:2}] {} ... ", idx + 1, title);

        match lex_safe(code) {
            Ok(tokens) => {
                let input_tokens: Vec<(usize, hulk::Token, usize)> = tokens
                    .into_iter()
                    .enumerate()
                    .map(|(i, token)| (i, token, i + 1))
                    .collect();

                match parser.parse(input_tokens.into_iter()) {
                    Ok(ast) => {
                        println!("✓ OK");
                        println!("{}", "-".repeat(80));
                        println!("AST DETALLADO:");
                        print_ast(&ast);
                        println!("{}", "-".repeat(80));
                        println!();
                        valid_count += 1;
                    }
                    Err(e) => {
                        println!("✗ ERROR (Parse)");
                        error_count += 1;
                        println!("    {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("✗ ERROR (Lex)");
                error_count += 1;
                println!("    {}", e);
            }
        }
    }

    println!();
    println!("{}", "=".repeat(80));
    println!("RESUMEN:");
    println!("  Total de ejemplos: {}", examples.len());
    println!("  Válidos: {} ✓", valid_count);
    println!("  Inválidos: {} ✗", error_count);

    println!();
    println!("{}", "=".repeat(80));

    Ok(())
}

fn print_ast(program: &hulk::Program) {
    println!("Program with {} items:\n", program.items.len());
    for (i, item) in program.items.iter().enumerate() {
        match item {
            hulk::Item::Function(func) => {
                println!("  [{}] Function: {}", i, func.name);
                println!("      Parameters: {}", func.params.len());
                for (p_idx, param) in func.params.iter().enumerate() {
                    let mut type_str = if let Some(typ) = &param.types {
                        format_type_ref(typ)
                    } else {
                        "untyped".to_string()
                    };
                    if param.is_variadic {
                        type_str.push('*');
                    }
                    println!(
                        "        [{}] {} : {} (is_variadic: {})",
                        p_idx, param.name, type_str, param.is_variadic
                    );
                }
                let return_type_str = if let Some(ret_type) = &func.return_type {
                    format_type_ref(ret_type)
                } else {
                    "untyped".to_string()
                };
                println!("      Return type: {}", return_type_str);
                println!("      Body:");
                print_expr_details(&func.body, 8);
                println!();
            }
            hulk::Item::Type(typ) => {
                println!("  [{}] Type: {}", i, typ.name);
                if !typ.param.is_empty() {
                    println!("      Parameters: {}", typ.param.len());
                    for (p_idx, param) in typ.param.iter().enumerate() {
                        let mut type_str = if let Some(typ) = &param.types {
                            format_type_ref(typ)
                        } else {
                            "untyped".to_string()
                        };
                        if param.is_variadic {
                            type_str.push('*');
                        }
                        println!(
                            "        [{}] {} : {} (is_variadic: {})",
                            p_idx, param.name, type_str, param.is_variadic
                        );
                    }
                }
                if let Some(parent) = &typ.parent {
                    let parent_str = format_type_ref(parent);
                    println!("      Inherits from: {}", parent_str);
                    if !typ.parent_arg.is_empty() {
                        println!("      Parent arguments: {}", typ.parent_arg.len());
                        for (arg_idx, arg_expr) in typ.parent_arg.iter().enumerate() {
                            println!("        [{}]", arg_idx);
                            print_expr_details(arg_expr, 10);
                        }
                    }
                }
                println!("      Fields: {}", typ.fields.len());
                for (f_idx, field) in typ.fields.iter().enumerate() {
                    let type_str = if let Some(typ) = &field.type_annotation {
                        format_type_ref(typ)
                    } else {
                        "untyped".to_string()
                    };
                    println!(
                        "        [{}] name: {} , Type {}",
                        f_idx, field.name, type_str
                    );
                    println!("            Initializer:");
                    print_expr_details(&field.initializer, 12);
                    println!();
                }
                println!("      Methods: {}", typ.methods.len());
                for (m_idx, method) in typ.methods.iter().enumerate() {
                    println!("        [{}] {}", m_idx, method.name);
                    println!("            Parameters: {}", method.params.len());
                    for (p_idx, param) in method.params.iter().enumerate() {
                        let mut type_str = if let Some(typ) = &param.types {
                            format_type_ref(typ)
                        } else {
                            "untyped".to_string()
                        };
                        if param.is_variadic {
                            type_str.push('*');
                        }
                        println!(
                            "              [{}] {} : {} (is_variadic: {})",
                            p_idx, param.name, type_str, param.is_variadic
                        );
                    }
                    let return_type_str = if let Some(ret_type) = &method.return_type {
                        format_type_ref(ret_type)
                    } else {
                        "untyped".to_string()
                    };
                    println!("            Return type: {}", return_type_str);
                    println!("            Body:");
                    print_expr_details(&method.body, 12);
                    println!();
                }
            }
            hulk::Item::Protocol(proto) => {
                println!("  [{}] Protocol: {}", i, proto.name);
                if let Some(parent) = &proto.parent {
                    println!("      Extends: {}", format_type_ref(parent));
                }
                println!("      Methods: {}", proto.methods.len());
                for (m_idx, method) in proto.methods.iter().enumerate() {
                    println!("        [{}] {}", m_idx, method.name);
                    println!("            Parameters: {}", method.params.len());
                    for (p_idx, param) in method.params.iter().enumerate() {
                        let mut type_str = param
                            .types
                            .as_ref()
                            .map(format_type_ref)
                            .unwrap_or_else(|| "untyped".to_string());

                        if param.is_variadic {
                            type_str.push('*');
                        }

                        println!(
                            "              [{}] {} : {} (is_variadic: {})",
                            p_idx, param.name, type_str, param.is_variadic
                        );
                    }
                    println!(
                        "            Return type: {}",
                        format_type_ref(&method.return_type)
                    );
                }
            }
            hulk::Item::Macro(macr) => {
                println!("  [{}] Macro: {}", i, macr.name);
                println!("      Parameters: {}", macr.params.len());
                for (p_idx, param) in macr.params.iter().enumerate() {
                    let type_str = param
                        .type_info
                        .as_ref()
                        .map(format_type_ref)
                        .unwrap_or_else(|| "untyped".to_string());

                    let kind_str = match param.kind {
                        hulk::MacroParamKind::Normal => "Normal",
                        hulk::MacroParamKind::Block => "Block (*)",
                        hulk::MacroParamKind::Symbolic => "Symbolic (@)",
                        hulk::MacroParamKind::Placeholder => "Placeholder ($)",
                    };

                    println!(
                        "        [{}] {} : {} (kind: {})",
                        p_idx, param.name, type_str, kind_str
                    );
                }
                println!("      Body:");
                print_expr_details(&macr.body, 8);
                println!();
            }
            hulk::Item::GlobalExpr(expr) => {
                println!("  [{}] Expression (global)", i);
                print_expr_details(expr, 6);
            }
        }
    }
    println!();
}

fn format_type_ref(typ: &hulk::TypeRef) -> String {
    match typ {
        hulk::TypeRef::Number => "Number".to_string(),
        hulk::TypeRef::String => "String".to_string(),
        hulk::TypeRef::Boolean => "Boolean".to_string(),
        hulk::TypeRef::Vector(inner) => format!("{}[]", format_type_ref(inner)),
        hulk::TypeRef::Function(params, ret) => {
            let params_str = params
                .iter()
                .map(format_type_ref)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({}) -> {}", params_str, format_type_ref(ret))
        }
        hulk::TypeRef::Custom(name) => name.clone(),
    }
}

fn print_expr_details(expr: &hulk::Expr, _indent: usize) {
    print_expr_tree(expr, "", true);
}

fn operator_name(op: &hulk::BinaryOperator) -> &'static str {
    match op {
        hulk::BinaryOperator::Add => "+",
        hulk::BinaryOperator::Sub => "-",
        hulk::BinaryOperator::Mul => "*",
        hulk::BinaryOperator::Div => "/",
        hulk::BinaryOperator::Mod => "%",
        hulk::BinaryOperator::Pow => "^",
        hulk::BinaryOperator::Equal => "==",
        hulk::BinaryOperator::NotEqual => "!=",
        hulk::BinaryOperator::Less => "<",
        hulk::BinaryOperator::Greater => ">",
        hulk::BinaryOperator::LessEqual => "<=",
        hulk::BinaryOperator::GreaterEqual => ">=",
        hulk::BinaryOperator::And => "&",
        hulk::BinaryOperator::Or => "|",
        hulk::BinaryOperator::Concat => "@",
        hulk::BinaryOperator::FullConcat => "@@",
    }
}

fn unary_name(op: &hulk::UnaryOperator) -> &'static str {
    match op {
        hulk::UnaryOperator::Not => "!",
        hulk::UnaryOperator::Negate => "-",
    }
}

fn print_pattern_tree(pattern: &hulk::Pattern, prefix: &str, is_last: bool) {
    let connector = if is_last { "└─ " } else { "├─ " };

    match pattern {
        hulk::Pattern::Identifier {
            name,
            type_restriction,
        } => {
            if let Some(typ) = type_restriction {
                println!(
                    "{}{}[Pattern::Identifier] {} : {}",
                    prefix,
                    connector,
                    name,
                    format_type_ref(typ)
                );
            } else {
                println!("{}{}[Pattern::Identifier] {}", prefix, connector, name);
            }
        }
        hulk::Pattern::Binary {
            left,
            operator,
            right,
        } => {
            println!(
                "{}{}[Pattern::Binary] op: {}",
                prefix,
                connector,
                operator_name(operator)
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Left:", new_prefix);
            let left_prefix = format!("{}│  ", new_prefix);
            print_pattern_tree(left, &left_prefix, true);
            println!("{}└─ Right:", new_prefix);
            let right_prefix = format!("{}   ", new_prefix);
            print_pattern_tree(right, &right_prefix, true);
        }
        hulk::Pattern::Unary { operator, operand } => {
            println!(
                "{}{}[Pattern::Unary] op: {}",
                prefix,
                connector,
                unary_name(operator)
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            print_pattern_tree(operand, &new_prefix, true);
        }
        hulk::Pattern::Literal(lit) => {
            let value = match lit {
                hulk::LiteralValue::Number(n) => format!("{}", n),
                hulk::LiteralValue::String(s) => format!("\"{}\"", s),
                hulk::LiteralValue::Bool(b) => format!("{}", b),
            };
            println!("{}{}[Pattern::Literal] {}", prefix, connector, value);
        }
        hulk::Pattern::Default => {
            println!("{}{}[Pattern::Default]", prefix, connector);
        }
    }
}

fn print_expr_tree(expr: &hulk::Expr, prefix: &str, is_last: bool) {
    let connector = if is_last { "└─ " } else { "├─ " };

    match &expr.kind {
        hulk::KindExpr::Literal(lit) => {
            let value = match &lit.value {
                hulk::LiteralValue::Number(n) => format!("{}", n),
                hulk::LiteralValue::String(s) => format!("\"{}\"", s),
                hulk::LiteralValue::Bool(b) => format!("{}", b),
            };
            println!("{}{}[Literal] {}", prefix, connector, value);
        }
        hulk::KindExpr::Variable(var) => {
            println!("{}{}[Variable] name: {}", prefix, connector, var.name);
        }
        hulk::KindExpr::Binary(bin) => {
            let op_str = operator_name(&bin.operator);
            println!("{}{}[Binary] op: {}", prefix, connector, op_str);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Left operand:", new_prefix);
            let left_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&bin.left, &left_prefix, true);
            println!("{}└─ Right operand:", new_prefix);
            let right_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&bin.right, &right_prefix, true);
        }
        hulk::KindExpr::Unary(un) => {
            let op_str = unary_name(&un.operator);
            println!("{}{}[Unary] op: {}", prefix, connector, op_str);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            print_expr_tree(&un.right, &new_prefix, true);
        }
        hulk::KindExpr::Call(call) => {
            println!(
                "{}{}[Function Call] {} arguments",
                prefix,
                connector,
                call.arguments.len()
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Function:", new_prefix);
            let func_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&call.callee, &func_prefix, true);
            if !call.arguments.is_empty() {
                println!("{}├─ Arguments:", new_prefix);
                let args_prefix = format!("{}│  ", new_prefix);
                for (idx, arg) in call.arguments.iter().enumerate() {
                    let is_last_arg = idx == call.arguments.len() - 1;
                    println!("{}├─ Arg[{}]:", args_prefix, idx);
                    let arg_prefix = format!("{}│  ", args_prefix);
                    print_expr_tree(arg, &arg_prefix, is_last_arg);
                }
            }
        }
        hulk::KindExpr::BaseCall(base_call) => {
            println!(
                "{}{}[Base Call] {} arguments",
                prefix,
                connector,
                base_call.arguments.len()
            );
            if !base_call.arguments.is_empty() {
                let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
                println!("{}└─ Arguments:", new_prefix);
                let args_prefix = format!("{}   ", new_prefix);
                for (idx, arg) in base_call.arguments.iter().enumerate() {
                    let is_last_arg = idx == base_call.arguments.len() - 1;
                    let arg_connector = if is_last_arg { "└─" } else { "├─" };
                    println!("{}{}Arg[{}]:", args_prefix, arg_connector, idx);
                    let arg_prefix = format!(
                        "{}{}   ",
                        args_prefix,
                        if is_last_arg { "   " } else { "│  " }
                    );
                    print_expr_tree(arg, &arg_prefix, is_last_arg);
                }
            }
        }
        hulk::KindExpr::MacroCall(macro_call) => {
            println!(
                "{}{}[Macro Call] {} with {} argument(s)",
                prefix,
                connector,
                macro_call.name,
                macro_call.arguments.len()
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });

            if !macro_call.arguments.is_empty() {
                println!("{}├─ Arguments:", new_prefix);
                let args_prefix = format!("{}│  ", new_prefix);
                for (idx, arg) in macro_call.arguments.iter().enumerate() {
                    let is_last_arg = idx == macro_call.arguments.len() - 1;
                    let arg_connector = if is_last_arg { "└─" } else { "├─" };
                    let kind_str = match arg.kind {
                        hulk::MacroParamKind::Normal => "normal",
                        hulk::MacroParamKind::Symbolic => "symbolic(@)",
                        hulk::MacroParamKind::Placeholder => "placeholder($)",
                        hulk::MacroParamKind::Block => "block(*)",
                    };
                    println!(
                        "{}{}Arg[{}] ({})",
                        args_prefix, arg_connector, idx, kind_str
                    );
                    let arg_prefix = format!(
                        "{}{}   ",
                        args_prefix,
                        if is_last_arg { "   " } else { "│  " }
                    );
                    print_expr_tree(&arg.value, &arg_prefix, true);
                }
            }

            if let Some(action) = &macro_call.action {
                println!("{}└─ Action:", new_prefix);
                let action_prefix = format!("{}   ", new_prefix);
                print_expr_tree(action, &action_prefix, true);
            }
        }
        hulk::KindExpr::Let(let_expr) => {
            println!(
                "{}{}[Let] {} variable(s)",
                prefix,
                connector,
                let_expr.bindings.len()
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            for (idx, binding) in let_expr.bindings.iter().enumerate() {
                let type_str = if let Some(typ) = &binding.types {
                    format_type_ref(typ)
                } else {
                    "untyped".to_string()
                };
                println!(
                    "{}├─ Binding[{}]: Variable name {} , Type {}",
                    new_prefix, idx, binding.name, type_str
                );
                let bind_prefix = format!("{}│  ", new_prefix);
                println!("{}├─ Value:", bind_prefix);
                let val_prefix = format!("{}│  ", bind_prefix);
                print_expr_tree(&binding.initializer, &val_prefix, true);
            }
            println!("{}└─ Body:", new_prefix);
            let body_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&let_expr.body, &body_prefix, true);
        }
        hulk::KindExpr::If(if_expr) => {
            println!("{}{}[If Expression]", prefix, connector);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Condition:", new_prefix);
            let cond_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&if_expr.condition, &cond_prefix, true);
            println!("{}├─ Then:", new_prefix);
            let then_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&if_expr.then_branch, &then_prefix, true);

            // Mostrar elif branches si existen
            if !if_expr.elif_branches.is_empty() {
                println!(
                    "{}├─ Elif branches ({}):",
                    new_prefix,
                    if_expr.elif_branches.len()
                );
                for (idx, (elif_cond, elif_body)) in if_expr.elif_branches.iter().enumerate() {
                    let elif_prefix = format!("{}│  ", new_prefix);
                    println!("{}├─ Elif[{}]:", elif_prefix, idx);
                    let elif_details_prefix = format!("{}│  ", elif_prefix);
                    println!("{}├─ Condition:", elif_details_prefix);
                    let elif_cond_prefix = format!("{}│  ", elif_details_prefix);
                    print_expr_tree(elif_cond, &elif_cond_prefix, true);
                    println!("{}└─ Body:", elif_details_prefix);
                    let elif_body_prefix = format!("{}   ", elif_details_prefix);
                    print_expr_tree(elif_body, &elif_body_prefix, true);
                }
            }

            println!("{}└─ Else:", new_prefix);
            let else_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&if_expr.else_branch, &else_prefix, true);
        }
        hulk::KindExpr::While(while_expr) => {
            println!("{}{}[While Loop]", prefix, connector);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Condition:", new_prefix);
            let cond_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&while_expr.condition, &cond_prefix, true);
            println!("{}└─ Body:", new_prefix);
            let body_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&while_expr.body, &body_prefix, true);
        }
        hulk::KindExpr::For(for_expr) => {
            println!(
                "{}{}[For Loop] variable: {}",
                prefix, connector, for_expr.variable
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Iterable:", new_prefix);
            let iter_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&for_expr.iterable, &iter_prefix, true);
            println!("{}└─ Body:", new_prefix);
            let body_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&for_expr.body, &body_prefix, true);
        }
        hulk::KindExpr::Block(block) => {
            println!(
                "{}{}[Block] {} expression(s)",
                prefix,
                connector,
                block.expressions.len()
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            for (idx, expr) in block.expressions.iter().enumerate() {
                println!("{}├─ Expr[{}]:", new_prefix, idx);
                let expr_prefix = format!("{}│  ", new_prefix);
                print_expr_tree(expr, &expr_prefix, idx == block.expressions.len() - 1);
            }
        }
        hulk::KindExpr::Array(arr) => {
            println!(
                "{}{}[Array] {} element(s)",
                prefix,
                connector,
                arr.elements.len()
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            for (idx, elem) in arr.elements.iter().enumerate() {
                println!("{}├─ Elem[{}]:", new_prefix, idx);
                let elem_prefix = format!("{}│  ", new_prefix);
                print_expr_tree(elem, &elem_prefix, idx == arr.elements.len() - 1);
            }
        }
        hulk::KindExpr::New(new_expr) => {
            println!(
                "{}{}[New Instance] type: {}",
                prefix, connector, new_expr.type_name
            );
            if !new_expr.arguments.is_empty() {
                let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
                println!("{}└─ Arguments:", new_prefix);
                let args_prefix = format!("{}   ", new_prefix);
                for (idx, arg) in new_expr.arguments.iter().enumerate() {
                    print_expr_tree(arg, &args_prefix, idx == new_expr.arguments.len() - 1);
                }
            }
        }
        hulk::KindExpr::Assign(assign_expr) => {
            println!("{}{}[Assignment] :=", prefix, connector);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Target:", new_prefix);
            let target_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&assign_expr.target, &target_prefix, true);
            println!("{}└─ Value:", new_prefix);
            let value_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&assign_expr.value, &value_prefix, true);
        }
        hulk::KindExpr::MemberAccess(member) => {
            println!(
                "{}{}[Member Access] field: {}",
                prefix, connector, member.field
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            print_expr_tree(&member.object, &new_prefix, true);
        }
        hulk::KindExpr::Index(index_expr) => {
            println!("{}{}[Index Access]", prefix, connector);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Object:", new_prefix);
            let obj_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&index_expr.object, &obj_prefix, true);
            println!("{}└─ Index:", new_prefix);
            let idx_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&index_expr.index, &idx_prefix, true);
        }
        hulk::KindExpr::ArrayComprehension(comp) => {
            println!("{}{}[Array Comprehension]", prefix, connector);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Expression:", new_prefix);
            let expr_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&comp.element, &expr_prefix, true);
            println!("{}├─ Variable: {}", new_prefix, comp.variable);
            println!("{}└─ Iterable:", new_prefix);
            let iter_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&comp.iterable, &iter_prefix, true);
        }
        hulk::KindExpr::Lambda(lambda) => {
            println!(
                "{}{}[Lambda] {} param(s)",
                prefix,
                connector,
                lambda.params.len()
            );
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            if lambda.return_type.is_some() {
                println!("{}├─ Return type: :Some", new_prefix);
            }
            println!("{}└─ Body:", new_prefix);
            let body_prefix = format!("{}   ", new_prefix);
            print_expr_tree(&lambda.body, &body_prefix, true);
        }
        hulk::KindExpr::Is(is_expr) => {
            println!("{}{}[Is Type Check]", prefix, connector);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Expression:", new_prefix);
            let expr_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&is_expr.expression, &expr_prefix, true);
            println!("{}└─ Type: (some type)", new_prefix);
        }
        hulk::KindExpr::As(as_expr) => {
            println!("{}{}[Type Cast]", prefix, connector);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Expression:", new_prefix);
            let expr_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&as_expr.expression, &expr_prefix, true);
            println!("{}└─ Target type: (some type)", new_prefix);
        }
        hulk::KindExpr::Match(match_expr) => {
            println!("{}{}[Match Expression]", prefix, connector);
            let new_prefix = format!("{}{}", prefix, if is_last { "   " } else { "│  " });
            println!("{}├─ Expression:", new_prefix);
            let expr_prefix = format!("{}│  ", new_prefix);
            print_expr_tree(&match_expr.expression, &expr_prefix, true);
            if !match_expr.cases.is_empty() {
                println!("{}├─ Cases ({}):", new_prefix, match_expr.cases.len());
                let cases_prefix = format!("{}│  ", new_prefix);
                for (idx, case_expr) in match_expr.cases.iter().enumerate() {
                    let is_last_case = idx == match_expr.cases.len() - 1;
                    let case_connector = if is_last_case { "└─" } else { "├─" };
                    println!("{}{}Case[{}]", cases_prefix, case_connector, idx);

                    let case_prefix = format!(
                        "{}{}  ",
                        cases_prefix,
                        if is_last_case { "   " } else { "│  " }
                    );
                    println!("{}├─ Pattern:", case_prefix);
                    let pattern_prefix = format!("{}│  ", case_prefix);
                    print_pattern_tree(&case_expr.pattern, &pattern_prefix, true);

                    println!("{}└─ Body:", case_prefix);
                    let body_prefix = format!("{}   ", case_prefix);
                    print_expr_tree(&case_expr.body, &body_prefix, true);
                }
            }
        }
    }
}
