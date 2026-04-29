use hulk::analyze_program;

#[test]
fn accepts_function_with_inferred_number_return() {
    let input = r#"
function addOne(x) => x + 1;
addOne(5)
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected function return type to be inferred as Number, got: {result:?}"
    );
}

#[test]
fn accepts_function_with_inferred_string_return() {
    let input = r#"
function greet(name) => "Hello, " @@ name;
greet("World")
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected function return type to be inferred as String, got: {result:?}"
    );
}

#[test]
fn accepts_function_with_inferred_boolean_return() {
    let input = r#"
function isPositive(x) => x > 0;
isPositive(5)
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected function return type to be inferred as Boolean, got: {result:?}"
    );
}

#[test]
fn accepts_method_with_inferred_return_type() {
    let input = r#"
type Counter {
  value: Number = 0;
    increment() => 1;
}

new Counter()
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected method return type to be inferred as Number, got: {result:?}"
    );
}

#[test]
fn accepts_function_with_if_expression_inferred_return() {
    let input = r#"
function abs(x) => if (x > 0) x else 0 - x;
abs(-5)
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected function return type to be inferred from if branches, got: {result:?}"
    );
}

#[test]
fn accepts_method_with_block_inferred_return() {
    let input = r#"
type Animal {
    speak(): Number => 1;
}

type Dog inherits Animal {
    speak() => 1;
}

new Dog()
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected method return type to be inferred from block, got: {result:?}"
    );
}

#[test]
fn rejects_function_call_with_wrong_return_type_usage() {
    let input = r#"
function getNumber() => 42;
let x: String = getNumber() in x;
"#;

    let diagnostics = analyze_program(input).expect_err("expected type error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("incompatible") || d.message.contains("recibe") || d.message.contains("String") || d.message.contains("Number")));
}
