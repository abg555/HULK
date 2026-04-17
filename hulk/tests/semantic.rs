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

#[test]
fn assigns_unique_node_ids_for_inferred_types() {
    let analysis = analyze_program("let x = 1 in x + 2").expect("expected semantic success");

    assert!(
        analysis.inferred_types.len() >= 4,
        "expected multiple expression ids, got {}",
        analysis.inferred_types.len()
    );
}

#[test]
fn rejects_invalid_assignment_target() {
    let diagnostics = analyze_program("(1 + 2) := 3")
        .expect_err("expected assignment target semantic error");

    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("lado izquierdo de ':='")));
}

#[test]
fn validates_member_access_against_type_shape() {
    let ok_program = r#"
type Foo {
  value: Number = 1;
}
let x = new Foo() in x.value
"#;
    assert!(analyze_program(ok_program).is_ok());

    let bad_program = r#"
type Foo {
  value: Number = 1;
}
let x = new Foo() in x.missing
"#;
    let diagnostics = analyze_program(bad_program).expect_err("expected missing member error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("no define el miembro")));
}

#[test]
fn validates_constructor_arguments() {
    let input = r#"
type Point(x: Number, y: Number) {
  value: Number = 0;
}
new Point(1)
"#;
    let diagnostics = analyze_program(input).expect_err("expected constructor arity error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Constructor de Point espera")));
}

#[test]
fn accepts_type_conforming_to_protocol() {
        let input = r#"
protocol Printable {
    show(): String;
}

type Person {
    show() => "ok";
}

function render(x: Printable): String => x.show();
render(new Person())
"#;

        let result = analyze_program(input);
        assert!(result.is_ok(), "expected protocol conformance, got: {result:?}");
}

#[test]
fn rejects_type_missing_protocol_member() {
        let input = r#"
protocol Printable {
    show(): String;
}

type Person {
    name: String = "Ana";
}

function render(x: Printable): String => x.show();
render(new Person())
"#;

        let diagnostics = analyze_program(input).expect_err("expected protocol conformance error");
    assert!(diagnostics.iter().any(|d| {
        d.message.contains("Argumento 1 incompatible")
            || d.message.contains("no define el miembro show")
    }));
}

#[test]
fn rejects_non_exhaustive_boolean_match() {
    let input = r#"
let x: Boolean = true in match x {
  case true => 1;
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected non-exhaustive match error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Boolean no exhaustivo")));
}

#[test]
fn accepts_exhaustive_boolean_match_with_default() {
    let input = r#"
let x: Boolean = true in match x {
  case true => 1;
  default => 0;
}
"#;

    let result = analyze_program(input);
    assert!(result.is_ok(), "expected valid exhaustive match, got: {result:?}");
}

#[test]
fn rejects_incompatible_match_literal_pattern() {
    let input = r#"
let x: Number = 42 in match x {
  case "hello" => 1;
  default => 0;
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected incompatible pattern error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("Literal de patron incompatible")));
}

#[test]
fn narrows_scrutinee_variable_in_match_branch() {
        let input = r#"
type Animal {}

type Dog inherits Animal {
    bark(): String => "woof";
}

let a: Animal = new Dog() in match a {
    case d: Dog => a.bark();
    default => "none";
}
"#;

        let result = analyze_program(input);
        assert!(result.is_ok(), "expected branch narrowing to allow a.bark(), got: {result:?}");
}

#[test]
fn rejects_unreachable_case_after_default() {
        let input = r#"
match true {
    default => 0;
    case true => 1;
}
"#;

        let diagnostics = analyze_program(input).expect_err("expected unreachable case error");
        assert!(diagnostics
                .iter()
                .any(|d| d.message.contains("Caso inalcanzable")));
}

#[test]
fn rejects_duplicate_boolean_case_pattern() {
        let input = r#"
match true {
    case true => 1;
    case true => 2;
    case false => 3;
}
"#;

        let diagnostics = analyze_program(input).expect_err("expected duplicate case error");
        assert!(diagnostics
                .iter()
                .any(|d| d.message.contains("Patron duplicado: case true")));
}

#[test]
fn accepts_compatible_method_override() {
        let input = r#"
type Animal {
    speak(): String => "...";
}

type Dog inherits Animal {
    speak(): String => "woof";
}

let x: Animal = new Dog() in x.speak()
"#;

        let result = analyze_program(input);
        assert!(result.is_ok(), "expected compatible override, got: {result:?}");
}

#[test]
fn rejects_incompatible_method_override() {
        let input = r#"
type Animal {
    speak(): String => "...";
}

type Dog inherits Animal {
    speak(): Number => 1;
}

new Dog()
"#;

        let diagnostics = analyze_program(input).expect_err("expected override compatibility error");
        assert!(diagnostics
                .iter()
                .any(|d| d.message.contains("Override incompatible")));
}

#[test]
fn narrows_variable_type_with_is_in_if() {
    let input = r#"
type Animal {}

type Dog inherits Animal {
    bark(): String => "woof";
}

let a: Animal = new Dog() in if (a is Dog) a.bark() else "none"
"#;

        let result = analyze_program(input);
        assert!(result.is_ok(), "expected narrowing with is in if, got: {result:?}");
}

#[test]
fn rejects_incompatible_as_cast() {
    let input = r#"
let x: Number = 42 in x as String
"#;

        let diagnostics = analyze_program(input).expect_err("expected invalid cast error");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("Cast 'as' incompatible")));
}