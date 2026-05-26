use hulk::analyze_program;

#[test]
fn accepts_macro_expansion_in_expression() {
    let input = r#"
    def twice(x: Number) => x * 2;

    let y: Number = twice(21) in y;
    "#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected macro expansion to type-check, got: {result:?}"
    );
}

#[test]
fn rejects_type_mismatch_after_macro_expansion() {
    let input = r#"
    def add1(x: Number) => x + 1;

    let y: String = add1(2) in y;
    "#;

    let diagnostics =
        analyze_program(input).expect_err("expected macro expansion to expose the type mismatch");
    assert!(diagnostics.iter().any(|diag| {
        diag.message.contains("Binding")
            && diag.message.contains("esperaba String")
            && diag.message.contains("Number")
    }));
}

#[test]
fn accepts_trailing_block_macro() {
    let input = r#"
    def unless(cond: Boolean, *body) => if (!cond) body else 0;

    let flag: Boolean = false in
        let value: Number = unless(flag) {
            42;
        } in value;
    "#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected trailing block macro to type-check, got: {result:?}"
    );
}

#[test]
fn rejects_missing_trailing_block_for_block_macro_param() {
    let input = r#"
    def run(*body) => body;

    run();
    "#;

    let diagnostics = analyze_program(input).expect_err("expected missing trailing block error");
    assert!(diagnostics.iter().any(|diag| {
        diag.message.contains("run") && diag.message.contains("espera un bloque trailing")
    }));
}

#[test]
fn accepts_symbolic_macro_argument() {
    let input = r#"
    def identity(@value: Number) => value;

    let result: Number = identity(@40 + 2) in result;
    "#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected symbolic macro argument to type-check, got: {result:?}"
    );
}

#[test]
fn accepts_object_typed_symbolic_macro_argument() {
    let input = r#"
    def identity(@value: Object) => value;

    let item: Object = 42 in identity(@item);
    "#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected Object annotations in macros to type-check, got: {result:?}"
    );
}

#[test]
fn rejects_unmarked_symbolic_macro_argument() {
    let input = r#"
    def identity(@value: Number) => value;

    identity(1);
    "#;

    let diagnostics = analyze_program(input).expect_err("expected symbolic argument error");
    assert!(diagnostics.iter().any(|diag| {
        diag.message.contains("identity") && diag.message.contains("argumento simbolico")
    }));
}

#[test]
fn accepts_placeholder_macro_argument() {
    let input = r#"
    def read($name: Number) => name;

    let x: Number = 7 in read($x);
    "#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected placeholder macro argument to type-check, got: {result:?}"
    );
}

#[test]
fn rejects_placeholder_macro_argument_that_is_not_identifier() {
    let input = r#"
    def read($name: Number) => name;

    read($(1 + 2));
    "#;

    let diagnostics = analyze_program(input).expect_err("expected placeholder identifier error");
    assert!(diagnostics.iter().any(|diag| {
        diag.message.contains("name") && diag.message.contains("espera un identificador")
    }));
}

#[test]
fn rejects_recursive_macro_expansion() {
    let input = r#"
    def loop(x) => loop(x);

    loop(1);
    "#;

    let diagnostics = analyze_program(input).expect_err("expected recursive expansion error");
    assert!(diagnostics.iter().any(|diag| {
        diag.message.contains("Expansión recursiva") && diag.message.contains("loop")
    }));
}
