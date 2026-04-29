use hulk::analyze_program;

#[test]
fn rejects_type_mismatch_after_macro_expansion() {
    let input = r#"
    def add1(x: Number) => x + 1;

    let y: String = add1(2) in y;
    "#;

    let diagnostics = analyze_program(input).expect_err("expected macro expansion to expose the type mismatch");
    assert!(diagnostics.iter().any(|diag| {
        diag.message.contains("Binding")
            && diag.message.contains("esperaba String")
            && diag.message.contains("Number")
    }));
}
