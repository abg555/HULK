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
