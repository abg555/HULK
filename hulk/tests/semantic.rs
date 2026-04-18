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
fn rejects_non_boolean_match_without_default() {
        let input = r#"
let x: Number = 42 in match x {
    case 42 => 1;
}
"#;

        let diagnostics = analyze_program(input).expect_err("expected non-boolean non-exhaustive match error");
        assert!(diagnostics
                .iter()
                .any(|d| d.message.contains("Match no exhaustivo") && d.message.contains("falta default")));
}

#[test]
fn rejects_duplicate_string_case_pattern() {
        let input = r#"
match "a" {
    case "a" => 1;
    case "a" => 2;
    default => 0;
}
"#;

        let diagnostics = analyze_program(input).expect_err("expected duplicate string pattern error");
        assert!(diagnostics
                .iter()
                .any(|d| d.message.contains("Patron duplicado en match")));
}

#[test]
fn rejects_unreachable_typed_case_shadowed_by_parent_type() {
        let input = r#"
type Animal {}

type Dog inherits Animal {
    bark(): String => "woof";
}

let a: Animal = new Dog() in match a {
    case x: Animal => 1;
    case d: Dog => 2;
    default => 0;
}
"#;

        let diagnostics = analyze_program(input).expect_err("expected unreachable typed case error");
        assert!(diagnostics
                .iter()
                .any(|d| d.message.contains("Caso inalcanzable") && d.message.contains("cubre")));
}

#[test]
fn accepts_typed_case_before_parent_type_case() {
        let input = r#"
type Animal {}

type Dog inherits Animal {
    bark(): String => "woof";
}

let a: Animal = new Dog() in match a {
    case d: Dog => 2;
    case x: Animal => 1;
    default => 0;
}
"#;

        let diagnostics = analyze_program(input).expect_err("expected default-unreachable only");
        assert!(!diagnostics
                .iter()
                .any(|d| d.message.contains("Caso inalcanzable") && d.message.contains("Dog")));
}

    #[test]
    fn rejects_unreachable_literal_case_for_known_scrutinee() {
        let input = r#"
    match 42 {
      case 1 => 0;
      case 42 => 1;
    }
    "#;

        let diagnostics = analyze_program(input).expect_err("expected unreachable literal case");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("Caso inalcanzable")));
    }

    #[test]
    fn accepts_constant_match_without_default_when_one_case_always_matches() {
        let input = r#"
    match 42 {
      case 42 => 1;
    }
    "#;

        let result = analyze_program(input);
        assert!(result.is_ok(), "expected constant match to be exhaustive without default, got: {result:?}");
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

    #[test]
    fn rejects_typed_function_without_guaranteed_return_value() {
        let input = r#"
    function f(): Number => while (true) 1;
    f()
    "#;

        let diagnostics = analyze_program(input).expect_err("expected missing guaranteed return value");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("no garantiza valor en todos los caminos")));
    }

    #[test]
    fn accepts_typed_function_with_guaranteed_if_branches() {
        let input = r#"
    function f(x: Boolean): Number => if (x) 1 else 2;
    f(true)
    "#;

        let result = analyze_program(input);
        assert!(result.is_ok(), "expected guaranteed return value, got: {result:?}");
    }

    #[test]
    fn rejects_let_initializer_without_guaranteed_value() {
        let input = r#"
    let x: Number = while (true) 1 in x
    "#;

        let diagnostics = analyze_program(input).expect_err("expected invalid let initializer");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("inicializador de x no garantiza valor")));
    }

    #[test]
    fn rejects_field_initializer_without_guaranteed_value() {
        let input = r#"
    type A {
      n: Number = while (true) 1;
    }
    new A()
    "#;

        let diagnostics = analyze_program(input).expect_err("expected invalid field initializer");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("inicializador del campo n")));
    }

    #[test]
    fn rejects_assignment_rhs_without_guaranteed_value() {
        let input = r#"
    let x: Number = 0 in {
      x := while (true) 1;
      x
    }
    "#;

        let diagnostics = analyze_program(input).expect_err("expected invalid assignment rhs");
        assert!(diagnostics
            .iter()
            .any(|d| d.message.contains("expresion asignada no garantiza valor")));
    }

#[test]
fn reports_variable_maybe_uninitialized_after_partial_if_assignment() {
    let input = r#"
let x: Number = while (true) 1 in {
  if (true) x := 1 else 0;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn accepts_variable_initialized_in_all_if_branches() {
    let input = r#"
let x: Number = while (true) 1 in {
  if (true) x := 1 else x := 2;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected initializer error only");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("inicializador de x no garantiza valor")));
    assert!(!diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_while_assignment() {
    let input = r#"
let x: Number = while (true) 1 in {
  while (false) x := 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_for_with_unknown_iterable() {
    let input = r#"
let x: Number = while (true) 1 in {
    let ys = [1] in for (i in ys) x := i;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn accepts_variable_initialized_after_non_empty_array_for() {
    let input = r#"
let x: Number = while (true) 1 in {
  for (i in [1]) x := i;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected initializer error only");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("inicializador de x no garantiza valor")));
    assert!(!diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_empty_array_for() {
    let input = r#"
let x: Number = while (true) 1 in {
  for (i in []) x := i;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn accepts_variable_initialized_after_while_true_assignment() {
    let input = r#"
let x: Number = while (true) 1 in {
  while (true) x := 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected initializer error only");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("inicializador de x no garantiza valor")));
    assert!(!diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_while_false_assignment() {
    let input = r#"
let x: Number = while (true) 1 in {
  while (false) x := 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_while_with_unknown_condition() {
    let input = r#"
let x: Number = while (true) 1 in {
  while (rand() > 0) x := 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn accepts_variable_initialized_after_while_constant_true_condition() {
    let input = r#"
let x: Number = while (true) 1 in {
  while (1 < 2) x := 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected initializer error only");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("inicializador de x no garantiza valor")));
    assert!(!diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_while_constant_false_condition() {
    let input = r#"
let x: Number = while (true) 1 in {
  while (1 > 2) x := 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn accepts_variable_initialized_after_non_empty_constant_range_for() {
    let input = r#"
let x: Number = while (true) 1 in {
  for (i in range(0, 1)) x := i;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected initializer error only");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("inicializador de x no garantiza valor")));
    assert!(!diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_empty_constant_range_for() {
    let input = r#"
let x: Number = while (true) 1 in {
  for (i in range(0, 0)) x := i;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn accepts_variable_initialized_after_while_arithmetic_constant_true_condition() {
    let input = r#"
let x: Number = while (true) 1 in {
  while ((1 + 1) == (3 - 1)) x := 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected initializer error only");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("inicializador de x no garantiza valor")));
    assert!(!diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_while_arithmetic_constant_false_condition() {
    let input = r#"
let x: Number = while (true) 1 in {
  while ((2 * 3) < (5 - 1)) x := 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn accepts_variable_initialized_after_non_empty_arithmetic_range_for() {
    let input = r#"
let x: Number = while (true) 1 in {
  for (i in range(1 + 1, 5 - 2)) x := i;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected initializer error only");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("inicializador de x no garantiza valor")));
    assert!(!diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}

#[test]
fn reports_variable_maybe_uninitialized_after_empty_arithmetic_range_for() {
    let input = r#"
let x: Number = while (true) 1 in {
  for (i in range(2 * 2, 1 + 1)) x := i;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected maybe-uninitialized variable error");
    assert!(diagnostics
        .iter()
        .any(|d| d.message.contains("variable x puede no estar inicializada")));
}