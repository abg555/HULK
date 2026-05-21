use hulk::analyze_program;
use hulk::types::SemanticType;

#[test]
fn doc_print_hello_world() {
    let input = r#"print("Hello World");"#;
    let result = analyze_program(input);
    assert!(result.is_ok(), "expected no semantic errors, got: {result:?}");
}

#[test]
fn doc_arithmetic_expression() {
    let input = r#"print((((1 + 2) ^ 3) * 4) / 5);"#;
    assert!(analyze_program(input).is_ok());
}

#[test]
fn doc_string_concat() {
    let input = r#"print("The meaning of life is " @ 42);"#;
    assert!(analyze_program(input).is_ok());
}

#[test]
fn doc_builtin_math_expression() {
    let input = r#"print(sin(2 * PI) ^ (2 + cos((3 * PI) / log(4, 64))));"#;
    assert!(analyze_program(input).is_ok());
}

#[test]
fn doc_expression_block() {
    let input = r#"{ print(42); print(sin(PI / 2)); print("Hello World"); }"#;
    assert!(analyze_program(input).is_ok());
}

#[test]
fn doc_inline_functions_and_call() {
    let input = r#"
function tan(x) => sin(x) / cos(x);
function cot(x) => 1 / tan(x);
print(tan(PI) ^ 2 + cot(PI) ^ 2)
"#;
    assert!(analyze_program(input).is_ok());
}

#[test]
fn doc_let_simple_and_multiple() {
    let input = r#"let msg = "Hello World" in print(msg);"#;
    assert!(analyze_program(input).is_ok());

    let input2 = r#"let number = 42, text = "The meaning of life is" in print(text @ number);"#;
    assert!(analyze_program(input2).is_ok());
}

#[test]
fn doc_let_redefine_shadowing() {
    let input = r#"let a = 7, a = 7 * 6 in print(a);"#;
    assert!(analyze_program(input).is_ok());
}

#[test]
fn doc_for_loop_example() {
    let input = r#"let xs = range(0, 5) in for (x in xs) print(x)"#;
    assert!(analyze_program(input).is_ok());
}

#[test]
fn doc_while_example() {
    let input = r#"let a = 3 in while (a >= 0) { a := a - 1 };"#;
    assert!(analyze_program(input).is_ok());
}

#[test]
fn infers_function_param_types_from_body() {
    let src = r#"
function operate(x, y) {
    print(x + y);
    print(x - y);
    print(x * y);
    print(x / y);
}
"#;

    let ctx = analyze_program(src).expect("expected semantic success");
    let sym = ctx.global_symbols.get("operate").expect("operate symbol defined");
    match &sym.typ {
        SemanticType::Function(params, _ret) => {
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], SemanticType::Number);
            assert_eq!(params[1], SemanticType::Number);
        }
        other => panic!("expected operate to be a function, got {:?}", other),
    }
}