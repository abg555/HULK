use hulk::analyze_program;

#[test]
fn reports_undefined_identifier() {
    let input = "x + 1";
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Identificador no definido")));
}

#[test]
fn accepts_basic_typed_program() {
    let input = "function add(a: Number, b: Number): Number => a + b; add(1, 2)";

    let result = analyze_program(input);
    assert!(result.is_ok(), "expected no semantic errors, got: {result:?}");
}

#[test]
fn reports_arity_mismatch() {
    let input = "function add(a: Number, b: Number): Number => a + b; add(1)";
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Aridad invalida")));
}

#[test]
fn accepts_prelude_print_and_range() {
    let input = "let xs = range(0, 5) in for (x in xs) print(x)";

    let result = analyze_program(input);
    assert!(result.is_ok(), "expected no semantic errors, got: {result:?}");
}

#[test]
fn reports_range_type_errors() {
    let input = "range(\"0\", 5)";
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Argumento 1 incompatible")));
}

#[test]
fn reports_type_inheritance_cycle() {
    let input = r#"
type A inherits B {}
type B inherits A {}
"#;
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Ciclo detectado en herencia de tipos")));
}

#[test]
fn reports_protocol_extending_type() {
    let input = r#"
type Parent {}
protocol Child extends Parent {}
"#;
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("no es un protocolo")));
}

#[test]
fn reports_duplicated_type_method() {
    let input = r#"
type Foo {
  bar() => 1;
  bar() => 2;
}
"#;
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Metodo duplicado")));
}
