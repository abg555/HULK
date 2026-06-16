//! Casos esquinados del PARSER.
//!
//! Se ejercita `hulk::parse_program`, que tokeniza, parsea y expande macros.
//! Las pruebas se centran en sintaxis: precedencia/asociatividad de operadores,
//! anidamiento profundo, formas validas de cada construccion y entradas
//! malformadas que deben producir un diagnostico de parseo.

use hulk::parse_program;

fn parses(input: &str) {
    let result = parse_program(input);
    assert!(
        result.is_ok(),
        "se esperaba parseo exitoso para {input:?}, got: {:?}",
        result.err()
    );
}

fn fails(input: &str) {
    let result = parse_program(input);
    assert!(result.is_err(), "se esperaba error de parseo para {input:?}, pero parseo OK");
}

// ----------------------------------------------------------------------------
// Expresiones y precedencia
// ----------------------------------------------------------------------------

#[test]
fn arithmetic_precedence_and_associativity() {
    parses("1 + 2 * 3 - 4 / 2");
    parses("2 ^ 3 ^ 2");
    parses("(1 + 2) * (3 - 4)");
    parses("-5 + -3");
    parses("10 % 3 + 1");
}

#[test]
fn deeply_nested_parentheses() {
    parses("(((((((((1)))))))))");
    parses("((1 + 2) * ((3 + 4) - (5 * 6)))");
}

#[test]
fn comparison_and_logical_operators() {
    parses("1 < 2 & 3 > 2 | 4 == 4");
    parses("!(1 == 2) & !(3 != 3)");
    parses("1 <= 2 & 2 >= 1");
}

#[test]
fn string_concatenation_operators() {
    parses(r#""a" @ "b" @@ "c""#);
    parses(r#""x = " @ 42 @@ " fin""#);
}

#[test]
fn unary_minus_and_not_chains() {
    parses("- - - 5");
    parses("!!true");
}

// ----------------------------------------------------------------------------
// let / bloques
// ----------------------------------------------------------------------------

#[test]
fn let_single_and_multiple_bindings() {
    parses("let x = 1 in x");
    parses("let x = 1, y = 2, z = 3 in x + y + z");
    parses("let x: Number = 1 in x");
}

#[test]
fn nested_let_expressions() {
    parses("let x = 1 in let y = 2 in let z = 3 in x + y + z");
}

#[test]
fn block_with_multiple_statements_and_trailing_semicolon() {
    parses("{ print(1); print(2); print(3); }");
    parses("{ 1; 2; 3 }");
}

#[test]
fn empty_block_in_let_body() {
    parses("let x = 1 in { x; }");
}

// ----------------------------------------------------------------------------
// Control de flujo
// ----------------------------------------------------------------------------

#[test]
fn if_elif_else_chains() {
    parses("if (true) 1 else 2");
    parses("if (1 < 2) 1 elif (2 < 3) 2 elif (3 < 4) 3 else 4");
}

#[test]
fn while_and_for_loops() {
    parses("while (true) 1");
    parses("for (x in range(0, 10)) print(x)");
    parses("for (x in [1, 2, 3]) x");
}

#[test]
fn nested_control_flow() {
    parses("for (i in range(0, 3)) for (j in range(0, 3)) if (i < j) print(i) else print(j)");
}

#[test]
fn match_with_cases_and_default() {
    parses(r#"match 1 { case 1 => "one"; case 2 => "two"; default => "other"; }"#);
    parses("match x { case y: Number => y; default => 0; }");
}

// ----------------------------------------------------------------------------
// Funciones / lambdas / macros
// ----------------------------------------------------------------------------

#[test]
fn function_declarations_arrow_and_block() {
    parses("function f(x: Number): Number => x + 1; f(1)");
    parses("function g(x) => x; g(2)");
    parses("function h(): Number { 42 } h()");
}

#[test]
fn lambda_expressions() {
    parses("let f = (x: Number): Number => x * 2 in f(21)");
    parses("(x: Number): Boolean => x > 0");
}

#[test]
fn higher_order_and_chained_calls() {
    parses("f(g(h(1)))");
    parses("a.b().c().d()");
}

#[test]
fn function_type_annotation() {
    parses("function apply(f: (Number) -> Number, x: Number): Number => f(x); apply((y: Number): Number => y, 1)");
}

#[test]
fn macro_definition_forms() {
    parses("def twice(x: Number) => x * 2; twice(21)");
    parses("def read($name: Number) => name; let x: Number = 1 in read($x)");
    parses("def identity(@value: Number) => value; identity(@1 + 2)");
}

// ----------------------------------------------------------------------------
// Tipos / protocolos / objetos
// ----------------------------------------------------------------------------

#[test]
fn type_declarations_with_fields_and_methods() {
    parses("type Point { x: Number = 0; y: Number = 0; getX(): Number => self.x; } new Point()");
}

#[test]
fn type_inheritance_and_constructor_args() {
    parses("type A(x: Number) {} type B inherits A(1) {} new B()");
    parses("type A(x: Number, y: Number) {} type B(p: Number) inherits A(p, p) {} new B(3)");
}

#[test]
fn protocol_declaration_and_extends() {
    parses("protocol P { f(): Number; } protocol Q extends P { g(): String; }");
}

#[test]
fn new_member_access_and_method_chains() {
    parses("let p = new Point() in p.getX()");
    parses("self.x := self.x + 1");
}

#[test]
fn is_and_as_operators() {
    parses("let x = new A() in x is A");
    parses("let x = new A() in x as B");
}

// ----------------------------------------------------------------------------
// Vectores
// ----------------------------------------------------------------------------

#[test]
fn vector_literals_and_indexing() {
    parses("[1, 2, 3]");
    parses("[[1, 2], [3, 4]]");
    parses("let v = [1, 2, 3] in v[0]");
    parses("[]");
}

#[test]
fn vector_type_annotations() {
    parses("function id(v: Number[]): Number[] => v; id([1, 2, 3])");
}

#[test]
fn list_comprehension_syntax() {
    // Comprension de listas: [expr | var in iterable]
    parses("[x * 2 | x in range(0, 5)]");
}

// ----------------------------------------------------------------------------
// Imports
// ----------------------------------------------------------------------------

#[test]
fn import_statements_parse() {
    parses("import math\n42");
}

// ----------------------------------------------------------------------------
// Entradas malformadas (deben fallar)
// ----------------------------------------------------------------------------

#[test]
fn unbalanced_parentheses_fail() {
    fails("(1 + 2");
    fails("1 + 2)");
    fails("((1)");
}

#[test]
fn unbalanced_brackets_fail() {
    fails("[1, 2, 3");
    fails("let v = [1, 2 in v");
}

#[test]
fn unbalanced_braces_fail() {
    fails("{ 1; 2; ");
}

#[test]
fn dangling_binary_operator_fails() {
    fails("1 +");
    fails("* 3");
    fails("1 + * 2");
}

#[test]
fn if_without_branches_fails() {
    fails("if (true)");
    fails("if else 2");
}

#[test]
fn function_without_name_fails() {
    fails("function (x) => x");
}

#[test]
fn let_without_in_fails() {
    fails("let x = 1 x");
}

#[test]
fn empty_input_fails_to_parse() {
    // No hay expresion ni declaracion: parseo invalido.
    fails("");
}

#[test]
fn stray_keyword_fails() {
    fails("inherits");
    fails("else 1");
}
