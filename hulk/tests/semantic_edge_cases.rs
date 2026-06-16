//! Casos esquinados del ANALISIS SEMANTICO.
//!
//! Se ejercita `hulk::analyze_program`: inferencia de tipos, compatibilidad,
//! aridad, scoping/shadowing, herencia y overrides, protocolos, narrowing,
//! exhaustividad de match, analisis de flujo (codigo inalcanzable) y vectores.

use hulk::analyze_program;

fn ok(input: &str) {
    let result = analyze_program(input);
    assert!(result.is_ok(), "se esperaba programa valido, got: {result:?}");
}

fn err_contains(input: &str, needle: &str) {
    let diagnostics = analyze_program(input).expect_err("se esperaban errores semanticos");
    assert!(
        diagnostics.iter().any(|d| d.message.contains(needle)),
        "se esperaba un diagnostico con {needle:?}, got: {diagnostics:?}"
    );
}

// ----------------------------------------------------------------------------
// Tipos basicos / inferencia / compatibilidad
// ----------------------------------------------------------------------------

#[test]
fn accepts_arithmetic_and_string_concat() {
    ok("print((1 + 2) * 3 / 4 - 5 % 2)");
    ok(r#"print("a" @ 1 @@ "b")"#);
}

#[test]
fn rejects_arithmetic_on_string() {
    err_contains(r#""hello" + 1"#, "incompatible");
}

#[test]
fn rejects_boolean_in_arithmetic() {
    err_contains("true + 1", "incompatible");
}

#[test]
fn rejects_undefined_identifier() {
    err_contains("x + 1", "Identificador no definido");
}

#[test]
fn let_binding_type_annotation_mismatch() {
    err_contains("let x: String = 42 in x", "incompatible");
}

#[test]
fn infers_let_binding_type_from_initializer() {
    ok("let x = 1 + 2 in x * 4");
    ok(r#"let s = "abc" in s @ "def""#);
}

// ----------------------------------------------------------------------------
// Scoping / shadowing
// ----------------------------------------------------------------------------

#[test]
fn shadowing_in_let_rebinds_type() {
    // Se permite re-ligar la misma variable con un tipo distinto en el mismo let.
    ok(r#"let a = 7, a = "siete" in print(a)"#);
}

#[test]
fn nested_let_inner_shadows_outer() {
    ok("let x = 1 in let x = x + 1 in let x = x + 1 in x");
}

#[test]
fn variable_out_of_scope_is_rejected() {
    err_contains("{ let x = 1 in x; x }", "Identificador no definido");
}

// ----------------------------------------------------------------------------
// Funciones / aridad
// ----------------------------------------------------------------------------

#[test]
fn accepts_well_typed_function_call() {
    ok("function add(a: Number, b: Number): Number => a + b; add(1, 2)");
}

#[test]
fn rejects_too_few_arguments() {
    err_contains(
        "function add(a: Number, b: Number): Number => a + b; add(1)",
        "Aridad invalida",
    );
}

#[test]
fn rejects_too_many_arguments() {
    err_contains(
        "function id(a: Number): Number => a; id(1, 2, 3)",
        "Aridad invalida",
    );
}

#[test]
fn rejects_argument_type_mismatch() {
    err_contains(
        r#"function id(a: Number): Number => a; id("hello")"#,
        "incompatible",
    );
}

#[test]
fn accepts_recursive_function() {
    ok("function fact(n: Number): Number => if (n <= 1) 1 else n * fact(n - 1); fact(5)");
}

// ----------------------------------------------------------------------------
// Herencia / overrides / acceso a miembros
// ----------------------------------------------------------------------------

#[test]
fn rejects_inheritance_cycle() {
    err_contains(
        "type A inherits B {} type B inherits A {}",
        "Ciclo detectado en herencia de tipos",
    );
}

#[test]
fn rejects_duplicate_method() {
    err_contains("type Foo { bar() => 1; bar() => 2; }", "Metodo duplicado");
}

#[test]
fn rejects_missing_member_access() {
    err_contains(
        "type Foo { value: Number = 1; } let x = new Foo() in x.missing",
        "no define el miembro",
    );
}

#[test]
fn accepts_compatible_override() {
    ok(
        "type Animal { speak(): String => \"...\"; } \
         type Dog inherits Animal { speak(): String => \"woof\"; } \
         let x: Animal = new Dog() in x.speak()",
    );
}

#[test]
fn rejects_incompatible_override_return_type() {
    err_contains(
        "type Animal { speak(): String => \"...\"; } \
         type Dog inherits Animal { speak(): Number => 1; } new Dog()",
        "Override incompatible",
    );
}

#[test]
fn rejects_constructor_arity_mismatch() {
    err_contains(
        "type Point(x: Number, y: Number) { value: Number = 0; } new Point(1)",
        "Constructor de Point espera",
    );
}

#[test]
fn accepts_inherited_constructor_arguments() {
    ok(
        "type Point(x: Number, y: Number) { x: Number = x; y: Number = y; } \
         type PolarPoint inherits Point {} new PolarPoint(3, 4)",
    );
}

// ----------------------------------------------------------------------------
// Protocolos
// ----------------------------------------------------------------------------

#[test]
fn accepts_type_conforming_to_protocol() {
    ok(
        "protocol Printable { show(): String; } \
         type Person { show() => \"ok\"; } \
         function render(x: Printable): String => x.show(); render(new Person())",
    );
}

#[test]
fn rejects_protocol_extending_a_type() {
    err_contains(
        "type Parent {} protocol Child extends Parent {}",
        "no es un protocolo",
    );
}

// ----------------------------------------------------------------------------
// Narrowing con is / match
// ----------------------------------------------------------------------------

#[test]
fn narrows_with_is_in_if() {
    ok(
        "type Animal {} type Dog inherits Animal { bark(): String => \"woof\"; } \
         let a: Animal = new Dog() in if (a is Dog) a.bark() else \"none\"",
    );
}

#[test]
fn narrows_scrutinee_in_match_branch() {
    ok(
        "type Animal {} type Dog inherits Animal { bark(): String => \"woof\"; } \
         let a: Animal = new Dog() in match a { case d: Dog => a.bark(); default => \"none\"; }",
    );
}

#[test]
fn rejects_non_exhaustive_boolean_match() {
    err_contains(
        "let x: Boolean = true in match x { case true => 1; }",
        "Boolean no exhaustivo",
    );
}

#[test]
fn accepts_exhaustive_boolean_match_with_default() {
    ok("let x: Boolean = true in match x { case true => 1; default => 0; }");
}

#[test]
fn rejects_incompatible_match_literal_pattern() {
    err_contains(
        r#"let x: Number = 42 in match x { case "hello" => 1; default => 0; }"#,
        "Literal de patron incompatible",
    );
}

#[test]
fn rejects_duplicate_boolean_case() {
    err_contains(
        "match true { case true => 1; case true => 2; case false => 3; }",
        "Patron duplicado",
    );
}

#[test]
fn rejects_unreachable_case_after_default() {
    err_contains(
        "match true { default => 0; case true => 1; }",
        "Caso inalcanzable",
    );
}

// ----------------------------------------------------------------------------
// Analisis de flujo: codigo inalcanzable
// ----------------------------------------------------------------------------

#[test]
fn rejects_while_false_body() {
    err_contains("while (false) 1", "Cuerpo de while inalcanzable");
}

#[test]
fn rejects_unreachable_after_infinite_while() {
    err_contains(
        "let x: Number = 1 in { while (true) 1; x }",
        "Expresion inalcanzable",
    );
}

#[test]
fn rejects_unreachable_expression_after_non_terminating_match() {
    err_contains(
        "{ match true { default => while (true) 1; }; 1 }",
        "Expresion inalcanzable",
    );
}

// ----------------------------------------------------------------------------
// Asignaciones de solo lectura
// ----------------------------------------------------------------------------

#[test]
fn rejects_assignment_to_for_iterator() {
    err_contains("for (i in [1]) i := 2", "iterador de for");
}

#[test]
fn rejects_invalid_assignment_target() {
    err_contains("(1 + 2) := 3", "lado izquierdo de ':='");
}

#[test]
fn accepts_assignment_to_regular_variable() {
    ok("let acc: Number = 0 in { for (i in [1]) acc := i; acc }");
}

// ----------------------------------------------------------------------------
// Cast / Object
// ----------------------------------------------------------------------------

#[test]
fn rejects_incompatible_as_cast() {
    err_contains("let x: Number = 42 in x as String", "Cast 'as' incompatible");
}

#[test]
fn object_accepts_all_value_types() {
    ok(
        "type Box {} { let n: Object = 42 in print(n); let s: Object = \"hi\" in print(s); \
         let b: Object = true in print(b); let o: Object = new Box() in print(o); }",
    );
}

// ----------------------------------------------------------------------------
// Vectores
// ----------------------------------------------------------------------------

#[test]
fn accepts_vector_builtin_members() {
    ok(
        "let numbers = [1, 2, 3] in { let s: Number = numbers.size() in print(s); \
         let c: Number = numbers.current() in print(c); }",
    );
}

#[test]
fn rejects_unknown_vector_member() {
    err_contains(
        "let numbers = [1, 2, 3] in numbers.clear()",
        "vector no define el miembro clear",
    );
}

#[test]
fn joins_array_elements_to_nearest_parent() {
    ok(
        "type Animal { speak(): String => \"noise\"; } \
         type Dog inherits Animal {} type Cat inherits Animal {} \
         let animals = [new Dog(), new Cat()] in animals[0].speak()",
    );
}

#[test]
fn accepts_for_over_vector_with_typed_iterator() {
    ok("let items = [1, 2, 3] in let sum: Number = 0 in for (i in items) { sum := sum + i; print(i) }");
}

#[test]
fn rejects_iterator_type_mismatch() {
    err_contains(
        "type Words { next(): Boolean => true; current(): String => \"one\"; } \
         for (i in new Words()) { let n: Number = i in print(n) }",
        "Binding n incompatible",
    );
}

// ----------------------------------------------------------------------------
// Prelude / builtins
// ----------------------------------------------------------------------------

#[test]
fn accepts_math_constants_and_builtins() {
    ok("print(sin(2 * PI) + cos(E) + sqrt(16) + log(2, 8))");
}

#[test]
fn rejects_range_with_string_argument() {
    err_contains(r#"range("0", 5)"#, "Argumento 1 incompatible");
}

// ----------------------------------------------------------------------------
// Macros
// ----------------------------------------------------------------------------

#[test]
fn accepts_macro_expansion() {
    ok("def twice(x: Number) => x * 2; let y: Number = twice(21) in y");
}

#[test]
fn rejects_type_mismatch_after_macro_expansion() {
    err_contains(
        "def add1(x: Number) => x + 1; let y: String = add1(2) in y",
        "Binding",
    );
}

#[test]
fn rejects_recursive_macro() {
    err_contains("def loop(x) => loop(x); loop(1)", "recursiva");
}
