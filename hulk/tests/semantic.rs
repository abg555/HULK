use hulk::{analyze_program, semantic::types::SemanticType};
use std::fs;

#[test]
fn reports_undefined_identifier() {
    let input = "x + 1";
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Identificador no definido"))
    );
}

#[test]
fn accepts_basic_typed_program() {
    let input = "function add(a: Number, b: Number): Number => a + b; add(1, 2)";

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected no semantic errors, got: {result:?}"
    );
}

#[test]
fn accepts_type_argument_inferred_from_field_initializer() {
    let input = r#"
type A {
  f(): String => "ok";
}

type Box(x) {
  value: String = x.f();
}

new Box(new A())
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected no semantic errors, got: {result:?}"
    );
}

#[test]
fn accepts_let_binding_inferred_from_structural_use() {
    let input = r#"
type A {
  f(): String => "f";
  g(): String => "g";
}

function h(x) => let y = x in y.f() @@ y.g();
h(new A())
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected no semantic errors, got: {result:?}"
    );
}

#[test]
fn reports_arity_mismatch() {
    let input = "function add(a: Number, b: Number): Number => a + b; add(1)";
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Aridad invalida"))
    );
}

#[test]
fn accepts_prelude_print_and_range() {
    let input = "let xs = range(0, 5) in for (x in xs) print(x)";

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected no semantic errors, got: {result:?}"
    );
}

#[test]
fn accepts_prelude_math_constants() {
    let input = "print(PI + E)";

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected builtin math constants to be valid, got: {result:?}"
    );
}

#[test]
fn accepts_prelude_pi_constant() {
    let input = "print(PI * 2)";

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected builtin PI constant to be valid, got: {result:?}"
    );
}

#[test]
fn accepts_prelude_e_constant() {
    let input = "let x = E in print(x)";

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected builtin E constant to be valid, got: {result:?}"
    );
}

#[test]
fn reports_range_type_errors() {
    let input = "range(\"0\", 5)";
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Argumento 1 incompatible"))
    );
}

#[test]
fn reports_type_inheritance_cycle() {
    let input = r#"
type A inherits B {}
type B inherits A {}
"#;
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Ciclo detectado en herencia de tipos"))
    );
}

#[test]
fn reports_protocol_extending_type() {
    let input = r#"
type Parent {}
protocol Child extends Parent {}
"#;
    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("no es un protocolo"))
    );
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

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Metodo duplicado"))
    );
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
    let diagnostics =
        analyze_program("(1 + 2) := 3").expect_err("expected assignment target semantic error");

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("lado izquierdo de ':='"))
    );
}

#[test]
fn rejects_assignment_to_for_iterator_variable() {
    let input = r#"
for (i in [1]) i := 2
"#;

    let diagnostics =
        analyze_program(input).expect_err("expected readonly iterator assignment error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("No se puede asignar a i")
                && d.message.contains("iterador de for"))
    );
}

#[test]
fn rejects_assignment_to_match_pattern_binding() {
    let input = r#"
let x: Number = 1 in match x {
    case y => { y := 2; 0; };
    default => 0;
}
"#;

    let diagnostics =
        analyze_program(input).expect_err("expected readonly match binding assignment error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("No se puede asignar a y")
                && d.message.contains("patron de match"))
    );
}

#[test]
fn rejects_assignment_to_narrowed_match_scrutinee_alias() {
    let input = r#"
type Animal {}

type Dog inherits Animal {
    bark(): String => "woof";
}

let a: Animal = new Dog() in match a {
    case d: Dog => { a := new Dog(); "ok"; };
    default => "none";
}
"#;

    let diagnostics =
        analyze_program(input).expect_err("expected readonly narrowed alias assignment error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("No se puede asignar a a")
                && d.message.contains("estrechado de match"))
    );
}

#[test]
fn accepts_assignment_to_regular_variable_inside_for_body() {
    let input = r#"
let acc: Number = 0 in {
  for (i in [1]) acc := i;
  acc
}
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected assignment to regular variable to be valid, got: {result:?}"
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("no define el miembro"))
    );
}

#[test]
fn accepts_namespace_import_member_calls() {
    let module_path = "math.hulk";
    let module_source = r#"
function mlog(x) => x;
"#;

    fs::write(module_path, module_source).expect("failed to write temp math module");

    let input = r#"
import math

math.mlog(42)
"#;

    let result = analyze_program(input);
    fs::remove_file(module_path).ok();

    assert!(
        result.is_ok(),
        "expected namespace import member call to be valid, got: {result:?}"
    );
}

#[test]
fn reports_missing_imported_modules() {
    let input = r#"
import semanticmissingmodule
"#;

    let diagnostics = analyze_program(input).expect_err("expected missing module error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Modulo no encontrado")),
        "expected missing module diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn reports_import_cycles_between_modules() {
    let a_path = "semanticcyclea.hulk";
    let b_path = "semanticcycleb.hulk";

    fs::write(a_path, "import semanticcycleb\n").expect("failed to write cycle module A");
    fs::write(b_path, "import semanticcyclea\n").expect("failed to write cycle module B");

    let diagnostics =
        analyze_program("import semanticcyclea").expect_err("expected import cycle diagnostic");

    fs::remove_file(a_path).ok();
    fs::remove_file(b_path).ok();

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Ciclo de import detectado")),
        "expected import cycle diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn reports_nonexistent_exports() {
    let module_name = "semanticexporterrormodule";
    let module_path = format!("{}.hulk", module_name);
    let module_source = r#"
function publicfn() => 1;
export missingfn
"#;

    fs::write(&module_path, module_source).expect("failed to write temp module");

    let input = format!("import {}", module_name);
    let result = analyze_program(&input);
    fs::remove_file(&module_path).ok();

    assert!(
        result.is_err(),
        "expected export validation to fail, got: {result:?}"
    );

    let diagnostics = result.expect_err("expected export validation error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Export inexistente")),
        "expected nonexistent export diagnostic, got: {diagnostics:?}"
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Constructor de Point espera"))
    );
}
#[ignore]
#[test]
fn accepts_self_access_inside_method() {
    let input = r#"
type Animal {
    me() => self;
}

new Animal().me()
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected valid self access in method, got: {result:?}"
    );
}
#[ignore]
#[test]
fn rejects_self_outside_method() {
    let diagnostics = analyze_program("self").expect_err("expected invalid self usage");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("'self' solo es valido"))
    );
}
#[ignore]
#[test]
fn rejects_assignment_to_self_inside_method() {
    let input = r#"
type Animal {
    mutate() => self := new Animal();
}

new Animal().mutate()
"#;

    let diagnostics = analyze_program(input).expect_err("expected readonly self assignment error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("No se puede asignar a self")
                && d.message.contains("solo lectura"))
    );
}
#[ignore]
#[test]
fn accepts_valid_base_call_in_override() {
    let input = r#"
type Animal {
    foo() => 1;
}

type Dog inherits Animal {
    foo() => base();
}

new Dog().foo()
"#;

    let result = analyze_program(input);
    assert!(result.is_ok(), "expected valid base call, got: {result:?}");
}
#[ignore]
#[test]
fn rejects_base_call_outside_method() {
    let diagnostics = analyze_program("base(1)").expect_err("expected invalid base usage");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("'base(...)' solo es valido"))
    );
}
#[ignore]
#[test]
fn rejects_base_call_without_parent_type() {
    let input = r#"
type Animal {
    foo() => base();
}

new Animal().foo()
"#;

    let diagnostics = analyze_program(input).expect_err("expected missing parent base error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("base(...)") && d.message.contains("padre"))
    );
}
#[ignore]
#[test]
fn rejects_base_call_when_parent_method_missing() {
    let input = r#"
type Animal {
    bar() => 1;
}

type Dog inherits Animal {
    foo() => base();
}

new Dog().foo()
"#;

    let diagnostics =
        analyze_program(input).expect_err("expected missing parent method for base call");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("implementacion base") || d.message.contains("base(...)"))
    );
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
    assert!(
        result.is_ok(),
        "expected protocol conformance, got: {result:?}"
    );
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
fn accepts_functor_protocol_call_syntax() {
    let input = r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}

type IsOdd {
    invoke(x: Number): Boolean => x % 2 == 1;
}

function test(filter: NumberFilter): Boolean => filter(3);
test(new IsOdd())
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected functor protocol call syntax, got: {result:?}"
    );
}

#[test]
fn accepts_function_as_functor_protocol_argument() {
    let input = r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}

function is_odd(x: Number): Boolean => x % 2 == 1;
function test(filter: NumberFilter): Boolean => filter(3);
test(is_odd)
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected function to satisfy functor protocol, got: {result:?}"
    );
}

#[test]
fn accepts_lambda_as_functor_protocol_argument() {
    let input = r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}

function test(filter: NumberFilter): Boolean => filter(3);
test((x: Number): Boolean => x % 2 == 1)
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected lambda to satisfy functor protocol, got: {result:?}"
    );
}

#[test]
fn accepts_functor_object_where_function_type_is_expected() {
    let input = r#"
type IsOdd {
    invoke(x: Number): Boolean => x % 2 == 1;
}

function test(filter: (Number) -> Boolean): Boolean => filter(3);
test(new IsOdd())
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected functor object to satisfy function type, got: {result:?}"
    );
}

#[test]
fn accepts_invoke_member_on_function_type_functor_annotation() {
    let input = r#"
function test(filter: (Number) -> Boolean): Boolean => filter.invoke(3);
test((x: Number): Boolean => x % 2 == 1)
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected function type to expose invoke, got: {result:?}"
    );
}

#[test]
fn accepts_type_call_as_constructor_for_functor_examples() {
    let input = r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}

type IsOdd {
    invoke(x: Number): Boolean => x % 2 == 1;
}

function test(filter: NumberFilter): Boolean => filter(3);
test(IsOdd())
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected Type(...) constructor shorthand, got: {result:?}"
    );
}

#[test]
fn rejects_functor_call_with_wrong_arity() {
    let input = r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}

type IsOdd {
    invoke(x: Number): Boolean => x % 2 == 1;
}

function test(filter: NumberFilter): Boolean => filter();
test(new IsOdd())
"#;

    let diagnostics = analyze_program(input).expect_err("expected functor arity error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Aridad invalida"))
    );
}

#[test]
fn rejects_non_exhaustive_boolean_match() {
    let input = r#"
let x: Boolean = true in match x {
  case true => 1;
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected non-exhaustive match error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Boolean no exhaustivo"))
    );
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
    assert!(
        result.is_ok(),
        "expected valid exhaustive match, got: {result:?}"
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Literal de patron incompatible"))
    );
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
    assert!(
        result.is_ok(),
        "expected branch narrowing to allow a.bark(), got: {result:?}"
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Caso inalcanzable"))
    );
}

#[test]
fn rejects_unreachable_while_body_with_constant_false() {
    let input = r#"
while (false) 1
"#;

    let diagnostics = analyze_program(input).expect_err("expected unreachable while body error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Cuerpo de while inalcanzable"))
    );
}

#[test]
fn rejects_unreachable_expression_after_non_terminating_while() {
    let input = r#"
let x: Number = 1 in {
  while (true) 1;
  x
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected unreachable expression error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Expresion inalcanzable"))
    );
}

#[test]
fn rejects_unreachable_else_branch_when_if_condition_is_true() {
    let input = r#"
if (true) 1 else 2
"#;

    let diagnostics = analyze_program(input).expect_err("expected unreachable else branch error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Rama else inalcanzable"))
    );
}

#[test]
fn rejects_unreachable_then_branch_when_if_condition_is_false() {
    let input = r#"
if (false) 1 else 2
"#;

    let diagnostics = analyze_program(input).expect_err("expected unreachable then branch error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Rama then inalcanzable"))
    );
}

#[test]
fn rejects_unreachable_elif_when_if_condition_is_true() {
    let input = r#"
if (true) 1 elif (true) 2 else 3
"#;

    let diagnostics = analyze_program(input).expect_err("expected unreachable elif branch error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Rama elif inalcanzable"))
    );
}

#[test]
fn rejects_unreachable_else_when_elif_is_always_true() {
    let input = r#"
if (false) 1 elif (true) 2 else 3
"#;

    let diagnostics = analyze_program(input).expect_err("expected unreachable else branch error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Rama else inalcanzable"))
    );
}

#[test]
fn rejects_unreachable_expression_after_non_terminating_match_with_default() {
    let input = r#"
{
    match true {
        default => while (true) 1;
    };
    1
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected unreachable expression error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Expresion inalcanzable"))
    );
}

#[test]
fn rejects_unreachable_expression_after_non_terminating_exhaustive_boolean_match() {
    let input = r#"
{
    match true {
        case true => while (true) 1;
        case false => while (true) 1;
    };
    1
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected unreachable expression error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Expresion inalcanzable"))
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Patron duplicado: case true"))
    );
}

#[test]
fn rejects_non_boolean_match_without_default() {
    let input = r#"
let x: Number = 42 in match x {
    case 42 => 1;
}
"#;

    let diagnostics =
        analyze_program(input).expect_err("expected non-boolean non-exhaustive match error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Match no exhaustivo")
                && d.message.contains("falta default"))
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Patron duplicado en match"))
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Caso inalcanzable") && d.message.contains("cubre"))
    );
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
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.message.contains("Caso inalcanzable") && d.message.contains("Dog"))
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Caso inalcanzable"))
    );
}

#[test]
fn accepts_constant_match_without_default_when_one_case_always_matches() {
    let input = r#"
    match 42 {
      case 42 => 1;
    }
    "#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected constant match to be exhaustive without default, got: {result:?}"
    );
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
    assert!(
        result.is_ok(),
        "expected compatible override, got: {result:?}"
    );
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
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Override incompatible"))
    );
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
    assert!(
        result.is_ok(),
        "expected narrowing with is in if, got: {result:?}"
    );
}

#[test]
fn joins_if_branches_to_nearest_nominal_parent() {
    let input = r#"
type Animal {
    speak(): String => "noise";
}

type Dog inherits Animal {}
type Cat inherits Animal {}

let animal = if (rand() > 0) new Dog() else new Cat() in animal.speak()
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected if branches to join as Animal, got: {result:?}"
    );
}

#[test]
fn joins_array_elements_to_nearest_nominal_parent() {
    let input = r#"
type Animal {
    speak(): String => "noise";
}

type Dog inherits Animal {}
type Cat inherits Animal {}

let animals = [new Dog(), new Cat()] in animals[0].speak()
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected array element type to join as Animal, got: {result:?}"
    );
}

#[test]
fn object_accepts_builtin_and_custom_values() {
    let input = r#"
type Box {}

{
    let n: Object = 42 in print(n);
    let s: Object = "hello" in print(s);
    let b: Object = true in print(b);
    let o: Object = new Box() in print(o);
}
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected Object to accept all standard value types, got: {result:?}"
    );
}

#[test]
fn joins_unrelated_if_branches_to_object_not_unknown() {
    let input = r#"
let x: Number = if (rand() > 0) 1 else "text" in x
"#;

    let diagnostics = analyze_program(input).expect_err("expected Object/Number mismatch");
    assert!(
        diagnostics.iter().any(|d| {
            d.message.contains("Binding x incompatible")
                && d.message.contains("Number")
                && d.message.contains("Object")
        }),
        "expected incompatible Number/Object diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn protocols_can_use_object_in_signatures() {
    let input = r#"
protocol HasValue {
    value(): Object;
}

type NumberBox {
    value(): Number => 42;
}

let box: HasValue = new NumberBox() in print(box.value())
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected protocol return covariance through Object, got: {result:?}"
    );
}

#[test]
fn rejects_incompatible_as_cast() {
    let input = r#"
let x: Number = 42 in x as String
"#;

    let diagnostics = analyze_program(input).expect_err("expected invalid cast error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Cast 'as' incompatible"))
    );
}

#[test]
fn accepts_typed_function_with_while_expression_body() {
    let input = r#"
    function f(): Number => while (true) 1;
    f()
    "#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected while to be accepted as function body expression, got: {result:?}"
    );
}

#[test]
fn accepts_typed_function_with_guaranteed_if_branches() {
    let input = r#"
    function f(x: Boolean): Number => if (x) 1 else 2;
    f(true)
    "#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected guaranteed return value, got: {result:?}"
    );
}

#[test]
fn accepts_while_expression_in_value_positions() {
    let input = r#"
type A {
  n: Number = while (true) 1;
}

{
  let x: Number = while (true) 1 in print(x);
  let y: Number = 0 in {
    y := while (true) 2;
    print(y);
  };
  new A();
}
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected while expressions in value positions, got: {result:?}"
    );
}

#[test]
fn accepts_for_with_iterable_protocol_type() {
    // Verify that for-loop accepts vectors (which work with arrays)
    // Also verify that Iterable protocol is available for types to implement
    let input = r#"
let items = [1, 2, 3] in 
let sum: Number = 0 in
for (i in items) {
  sum := sum + i;
  print(i)
}
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected no semantic errors, got: {result:?}"
    );
}

#[test]
fn accepts_custom_type_implementing_iterable_protocol() {
    let input = r#"
type Counter {
  next(): Boolean => true;
  current(): Number => 1;
}

for (i in new Counter()) {
  let n: Number = i in print(n)
}
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected custom Iterable implementation to work, got: {result:?}"
    );
}

#[test]
fn rejects_for_over_custom_type_missing_iterable_methods() {
    let input = r#"
type NotIterable {}

for (i in new NotIterable()) print(i)
"#;

    let diagnostics = analyze_program(input).expect_err("expected Iterable conformance error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("implemente el protocolo Iterable")),
        "expected Iterable diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn rejects_for_iterator_use_when_current_type_is_incompatible() {
    let input = r#"
type Words {
  next(): Boolean => true;
  current(): String => "one";
}

for (i in new Words()) {
  let n: Number = i in print(n)
}
"#;

    let diagnostics = analyze_program(input).expect_err("expected iterator element type mismatch");
    assert!(
        diagnostics.iter().any(|d| {
            d.message.contains("Binding n incompatible")
                && d.message.contains("Number")
                && d.message.contains("String")
        }),
        "expected Number/String diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn accepts_vector_where_iterable_protocol_is_expected() {
    let input = r#"
let items: Iterable = [1, 2, 3] in print(items)
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected vectors to conform to Iterable, got: {result:?}"
    );
}

#[test]
fn accepts_vector_builtin_members() {
    let input = r#"
let numbers = [1, 2, 3] in {
  let size: Number = numbers.size() in print(size);
  let has_next: Boolean = numbers.next() in print(has_next);
  let current: Number = numbers.current() in print(current);
}
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected vector size/next/current members, got: {result:?}"
    );
}

#[test]
fn rejects_unknown_vector_member() {
    let input = r#"
let numbers = [1, 2, 3] in numbers.clear()
"#;

    let diagnostics = analyze_program(input).expect_err("expected unknown vector member error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("vector no define el miembro clear")),
        "expected unknown vector member diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn rejects_vector_size_after_erasing_to_iterable() {
    let input = r#"
let numbers: Iterable = [1, 2, 3] in numbers.size()
"#;

    let diagnostics = analyze_program(input).expect_err("expected Iterable member error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Iterable no define el miembro size")),
        "expected Iterable missing size diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn accepts_for_expression_in_value_positions() {
    let input = r#"
function first(): Number => for (i in [1]) i;

{
  let x: Number = for (i in [1]) i in print(x);
  let y: Number = 0 in {
    y := for (i in range(0, 1)) i;
    print(y);
  };
  first();
}
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected for expressions in value positions, got: {result:?}"
    );
}

#[test]
fn accepts_parent_constructor_args_valid() {
    let input = r#"
type Animal(x: Number) {}
type Dog inherits Animal(42) {}
new Dog()
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected valid parent constructor args, got: {result:?}"
    );
}

#[test]
fn accepts_parent_constructor_args_missing_as_implicit_inheritance() {
    let input = r#"
type Animal(x: Number) {}
type Dog inherits Animal() {}
new Dog(42)
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected empty parent args to behave like implicit constructor inheritance, got: {result:?}"
    );
}

#[test]
fn rejects_parent_constructor_args_arity() {
    let input = r#"
type Animal(x: Number, y: String) {}
type Dog inherits Animal(42) {}
new Dog()
"#;

    let diagnostics = analyze_program(input).expect_err("expected parent constructor arity error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Constructor de Animal espera 2"))
    );
}

#[test]
fn rejects_parent_constructor_args_incompatible_type() {
    let input = r#"
type Animal(x: Number) {}
type Dog inherits Animal("hello") {}
new Dog()
"#;

    let diagnostics = analyze_program(input).expect_err("expected parent constructor type error");
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Constructor padre Animal")
                && d.message.contains("incompatible"))
    );
}

#[test]
fn accepts_parent_without_ctor_args() {
    let input = r#"
type Animal {}
type Dog inherits Animal() {}
new Dog()
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected parent without ctor args to work, got: {result:?}"
    );
}

#[test]
fn accepts_implicit_and_explicit_constructor_inheritance() {
    let input = r#"
type Point(x: Number, y: Number) {
    x: Number = x;
    y: Number = y;

    getX(): Number => self.x;
    getY(): Number => self.y;
}

type PolarPoint inherits Point {
    rho(): Number => sqrt(self.getX() ^ 2 + self.getY() ^ 2);
}

type PolarPoint2(phi: Number, rho: Number) inherits Point(rho * sin(phi), rho * cos(phi)) {
    rho2(): Number => rho;
}

{
    let p = new PolarPoint(3, 4) in
        print("rho: " @ p.rho());

    let q = new PolarPoint2(1.0, 2.0) in
        print("rho2: " @ q.rho2());
}
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected no semantic errors for valid inheritance, got: {result:?}"
    );
}

#[test]
fn infers_number_type_from_attribute_initializers() {
    let input = r#"
        type Point {
            x = 0;
            y = 0;

            // Si x e y se infieren como Number, 
            // esta operación aritmética debe ser válida.
            add_coords(): Number => self.x + self.y;
        }

        let p = new Point() in p.add_coords()
    "#;

    let result = analyze_program(input);

    assert!(
        result.is_ok(),
        "Falló la inferencia de tipos para x e y: {result:?}"
    );
}

#[test]
fn accepts_corrected_polar_point_implementation() {
    let input = r#"
        type Point(ax: Number, ay: Number) {
            x: Number = ax;
            y: Number = ay;

            getX(): Number => self.x;
            getY(): Number => self.y;
        }

        type PolarPoint(ax: Number, ay: Number)
            inherits Point(ax, ay)
        {
            rho(): Number =>
                sqrt(self.getX() ^ 2 + self.getY() ^ 2);
        }

        type PolarPoint2(phi: Number, rho: Number)
            inherits Point(rho * sin(phi), rho * cos(phi))
        {
            r: Number = rho;

            rho2(): Number => self.r;
        }

        {
            let p = new PolarPoint(3, 4) in
                p.rho();

            let q = new PolarPoint2(1.0, 2.0) in
                q.rho2();
        }
    "#;

    let result = analyze_program(input);

    assert!(
        result.is_ok(),
        "El compilador debería aceptar la implementación corregida. Error: {:?}",
        result.err()
    );
}

#[test]
fn accepts_inherited_constructor_arguments() {
    let input = r#"
        type Point(x: Number, y: Number) {
        x: Number = x;
        y: Number = y;

        getX(): Number => self.x;
        getY(): Number => self.y;
    }

    type PolarPoint inherits Point {
        rho(): Number => sqrt(self.getX() ^ 2 + self.getY() ^ 2);
    }

    type PolarPoint2(phi: Number, rho: Number) inherits Point(rho * sin(phi), rho * cos(phi)) {
        rho2(): Number => rho;
    }

    {
        let p = new PolarPoint(3, 4) in
            print("rho: " @ p.rho());

        let q = new PolarPoint2(1.0, 2.0) in
            print("rho2: " @ q.rho2());
    }
    "#;

    let result = analyze_program(input);

    assert!(
        result.is_ok(),
        "El compilador debería aceptar la implementación corregida. Error: {:?}",
        result.err()
    );
}

#[test]
fn infers_point_fields_as_number() {
    let input = r#"
        type Point {
            x = 0;
            y = 0;

            getX() => self.x;
            getY() => self.y;

            setX(x) => self.x := x;
            setY(y) => self.y := y;
        }

        {
            let p = new Point() in {
                p.setX(42);
                p.setY(100);

                p.getX() + p.getY();
            };
        }
    "#;

    let result = analyze_program(input);

    assert!(
        result.is_ok(),
        "x e y deberían inferirse como Number. Error: {:?}",
        result.err()
    );
    let analysis = result.expect("expected Point program to be valid");
    let point = analysis
        .type_shapes
        .get("Point")
        .expect("Point shape should be present");

    assert_eq!(point.fields.get("x"), Some(&SemanticType::Number));
    assert_eq!(point.fields.get("y"), Some(&SemanticType::Number));
    assert_eq!(
        point.methods.get("getX"),
        Some(&SemanticType::Function(
            Vec::new(),
            Box::new(SemanticType::Number)
        ))
    );
    assert_eq!(
        point.methods.get("getY"),
        Some(&SemanticType::Function(
            Vec::new(),
            Box::new(SemanticType::Number)
        ))
    );
    assert_eq!(
        point.methods.get("setX"),
        Some(&SemanticType::Function(
            vec![SemanticType::Number],
            Box::new(SemanticType::Number)
        ))
    );
    assert_eq!(
        point.methods.get("setY"),
        Some(&SemanticType::Function(
            vec![SemanticType::Number],
            Box::new(SemanticType::Number)
        ))
    );
}

#[test]
fn rejects_non_number_assignment_to_inferred_point_fields() {
    let input = r#"
        type Point {
            x = 0;
            y = 0;

            getX() => self.x;
            getY() => self.y;

            setX(x) => self.x := x;
            setY(y) => self.y := y;
        }

        {
            let p = new Point() in {
                p.setX("hello");
            };
        }
    "#;

    let result = analyze_program(input);

    assert!(
        result.is_err(),
        "El compilador debería rechazar asignar String a un campo inferido como Number"
    );
}

#[test]
fn accepts_print_new_object_using_inherited_tostring() {
    let input = r#"
type Dog {
    name: String = "Fido";
}

print(new Dog())
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected print(new Dog()) to be valid via Object.toString inheritance, got: {result:?}"
    );
}

#[test]
fn accepts_valid_tostring_override_signature() {
    let input = r#"
type Dog {
    toString(): String => "Dog";
}

print(new Dog())
"#;

    let result = analyze_program(input);
    assert!(
        result.is_ok(),
        "expected valid toString override to pass semantic analysis, got: {result:?}"
    );
}

#[test]
fn reports_invalid_tostring_override_signature() {
    let input = r#"
type Dog {
    toString(x: Number): String => "Dog";
}

print(new Dog())
"#;

    let diagnostics = analyze_program(input).expect_err("expected semantic errors");

    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("Override incompatible") && d.message.contains("toString")),
        "expected override compatibility error for invalid toString signature, got: {diagnostics:?}"
    );
}
