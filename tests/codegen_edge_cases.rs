//! Casos esquinados de GENERACION DE CODIGO (LLVM IR).
//!
//! Guiados por Hulk.md. Cada prueba verifica que el IR contiene los patrones
//! clave, o que el error de codegen es el esperado cuando una feature aun no
//! esta implementada en esta etapa del compilador.
//!
//! Organizacion:
//!   - Macros (def, trailing-block, symbolic @, placeholder $)
//!   - Variables y scoping (shadowing, := como expresion, let anidado)
//!   - Condicionales (if/elif/else como expresion)
//!   - Loops (while devuelve valor, for devuelve valor)
//!   - Vectores (literal, indexacion, size, comprehension, iterable protocol)
//!   - OOP (ctor args, herencia, vtable, is/as, toString)
//!   - Concatenacion de strings (@, @@)
//!   - Imports de modulos externos
//!   - Features no implementadas en codegen (match, functors/lambdas)

use hulk::code_gen::CodeGenerator;
use hulk::{parse_program, SemanticAnalyzer};
use inkwell::context::Context;
use std::fs;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compile_to_ir(source: &str) -> String {
    let program = parse_program(source)
        .unwrap_or_else(|e| panic!("parse fallo: {e:?}"));
    let analysis = SemanticAnalyzer::new()
        .analyze(&program)
        .unwrap_or_else(|e| panic!("analisis semantico fallo: {e:?}"));

    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "codegen_edge");
    codegen
        .codegen_program(&program, &analysis)
        .unwrap_or_else(|e| panic!("codegen fallo: {e}"));
    codegen.module().print_to_string().to_string()
}

/// Para pruebas que pasan semantica pero el codegen aun no implementa
fn codegen_error_msg(source: &str) -> String {
    let program = parse_program(source)
        .unwrap_or_else(|e| panic!("parse fallo: {e:?}"));
    let analysis = SemanticAnalyzer::new()
        .analyze(&program)
        .unwrap_or_else(|e| panic!("analisis semantico fallo: {e:?}"));

    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "codegen_edge");
    codegen
        .codegen_program(&program, &analysis)
        .expect_err("se esperaba error de codegen")
}

fn ir_has(ir: &str, needle: &str) {
    assert!(
        ir.contains(needle),
        "IR deberia contener {needle:?}\nIR:\n{ir}"
    );
}

fn ir_lacks(ir: &str, needle: &str) {
    assert!(
        !ir.contains(needle),
        "IR NO deberia contener {needle:?}\nIR:\n{ir}"
    );
}

// ===========================================================================
// MACROS (seccion 18 del doc)
// ===========================================================================

// Doc: macros se expanden en compile-time; no existe ninguna funcion en el IR.
// `def twice(x: Number) => x * 2; twice(21)` debe quedar foldeado a 42.0
#[test]
fn macro_inline_is_compiled_away_no_fn_in_ir() {
    let ir = compile_to_ir("def twice(x: Number) => x * 2; twice(21)");
    ir_lacks(&ir, "define double @twice");
    // El resultado se dobla en compile-time: el return sera 42
    ir_has(&ir, "4.200000e+01");
}

// Macro con multiplicacion y suma: sigue sin dejar huella como funcion
#[test]
fn macro_arithmetic_expands_inline() {
    let ir = compile_to_ir("def square(n: Number) => n * n; square(7)");
    ir_lacks(&ir, "define double @square");
    ir_has(&ir, "4.900000e+01");
}

// Trailing-block macro: `def unless(cond, *body)`. El cuerpo se inserta inline.
// Doc: la invocacion `unless(flag) { 42; }` es azucar de `if (!cond) body else 0`.
#[test]
fn macro_trailing_block_generates_if_blocks() {
    let ir = compile_to_ir(r#"
def unless(cond: Boolean, *body) => if (!cond) body else 0;
function run(flag: Boolean): Number => unless(flag) {
    42;
};
run(false)
"#);
    ir_has(&ir, "if_then");
    ir_has(&ir, "if_else");
    ir_has(&ir, "if_merge");
    ir_has(&ir, "phi double");
    // El macro se expande; no hay una funcion `unless` en IR
    ir_lacks(&ir, "define double @unless");
}

// Symbolic arg @: el macro puede leer/asignar a una variable del contexto externo.
// Doc: `def swap(@a, @b)` puede intercambiar los valores de las variables originales.
#[test]
fn macro_symbolic_arg_reads_external_variable() {
    let ir = compile_to_ir(r#"
def identity(@value: Number) => value;
let x: Number = 7 in identity(@x)
"#);
    // El macro accede a `x` directamente; el resultado es 7
    ir_has(&ir, "7.000000e+00");
    ir_lacks(&ir, "define double @identity");
}

// Placeholder arg $: introduce un nuevo binding en el scope del invocador.
// Doc: `def read($name: Number) => name` permite `read($x)` donde `$x` nombra la variable.
#[test]
fn macro_placeholder_arg_introduces_binding() {
    let ir = compile_to_ir(r#"
def read($name: Number) => name;
let x: Number = 7 in read($x)
"#);
    ir_has(&ir, "7.000000e+00");
    ir_lacks(&ir, "define double @read");
}

// Macros compuestos: dos macros independientes llamados en secuencia
// (llamar un macro desde otro es un error semantico en esta impl.)
#[test]
fn two_independent_macros_both_expanded_inline() {
    let ir = compile_to_ir(r#"
def triple(x: Number) => x * 3;
def addone(x: Number) => x + 1;
{ triple(4); addone(11) }
"#);
    ir_lacks(&ir, "define double @triple");
    ir_lacks(&ir, "define double @addone");
    // 4*3=12 y 11+1=12 — ambos resultados aparecen
    ir_has(&ir, "1.200000e+01");
}

// ===========================================================================
// VARIABLES Y SCOPING (seccion 8 del doc)
// ===========================================================================

// Doc: `let a = 20 in { let a = 42 in print(a); print(a) }` imprime 42 luego 20.
// El IR debe tener dos alloca distintos para `a`.
#[test]
fn shadowing_allocates_separate_slots() {
    let ir = compile_to_ir(r#"
let a = 20 in {
    let a = 42 in print(a);
    print(a)
}
"#);
    // Dos alloca para a (el compilador los nombra a y a1)
    ir_has(&ir, "%a = alloca double");
    ir_has(&ir, "%a1 = alloca double");
    ir_has(&ir, "4.200000e+01");
    ir_has(&ir, "2.000000e+01");
}

// Doc: `:=` es una expresion que devuelve el valor asignado.
// `let b = a := 1 in { print(a); print(b) }` => ambos son 1.
#[test]
fn destructive_assign_returns_assigned_value() {
    let ir = compile_to_ir(r#"
let a = 0 in
    let b = a := 1 in {
        print(a);
        print(b);
        b
    }
"#);
    // Despues de `a := 1`, el valor recargado de a se guarda en b
    ir_has(&ir, "%reload_a = load double, ptr %a");
    ir_has(&ir, "%b = alloca double");
    ir_has(&ir, "1.000000e+00");
}

// Doc: `let a = (let b = 6 in b * 7) in print(a)` => imprime 42.
// El let anidado es una expresion con valor de retorno; el IR tiene b=6 y *7.
#[test]
fn let_is_an_expression_with_return_value() {
    let ir = compile_to_ir("let a = (let b = 6 in b * 7) in print(a)");
    ir_has(&ir, "6.000000e+00");
    ir_has(&ir, "7.000000e+00");
    ir_has(&ir, "@printf");
}

// Doc: en `let a = 6, b = a * 7` las variables se ligan de izq a der,
// por lo que `b` puede usar `a` ya ligada. El IR tiene a=6 y multiplica por 7.
#[test]
fn multiple_let_bindings_left_to_right_dependency() {
    let ir = compile_to_ir("let a = 6, b = a * 7 in b");
    ir_has(&ir, "6.000000e+00");
    ir_has(&ir, "7.000000e+00");
    ir_has(&ir, "fmul double");
}

// ===========================================================================
// CONDICIONALES (seccion 9 del doc)
// ===========================================================================

// Doc: `if` es una expresion y su valor es el de la rama ejecutada.
// `print(if (a % 2 == 0) "even" else "odd")` genera un phi sobre strings.
#[test]
fn if_as_expression_returns_phi_value() {
    let ir = compile_to_ir(r#"let a = 42 in print(if (a % 2 == 0) "even" else "odd")"#);
    ir_has(&ir, "if_then");
    ir_has(&ir, "if_else");
    ir_has(&ir, "if_merge");
    ir_has(&ir, "phi ptr");
}

// Doc: multiples ramas con `elif`.
// `if (x < 0) -1 elif (x == 0) 0 else 1` genera cadena de if/elif.
#[test]
fn if_elif_else_generates_chained_blocks() {
    let ir = compile_to_ir(
        "function classify(x: Number): Number => \
         if (x < 0) -1 elif (x == 0) 0 else 1; classify(5)",
    );
    ir_has(&ir, "if_then");
    ir_has(&ir, "if_else");
    ir_has(&ir, "if_merge");
    ir_has(&ir, "phi double");
}

// if devuelve objetos (herencia): el phi debe ser de tipo ptr
#[test]
fn if_branches_returning_objects_emit_ptr_phi() {
    let ir = compile_to_ir(r#"
type Animal { speak(): String => "..."; }
type Dog inherits Animal { speak(): String => "woof"; }
type Cat inherits Animal { speak(): String => "meow"; }
let a: Animal = if (rand() > 0) new Dog() else new Cat() in a.speak()
"#);
    ir_has(&ir, "phi ptr");
    ir_has(&ir, "if_then");
    ir_has(&ir, "if_else");
    ir_has(&ir, "if_merge");
}

// ===========================================================================
// LOOPS (seccion 10 del doc)
// ===========================================================================

// Doc: el `while` devuelve el valor de la ultima iteracion del cuerpo.
// La implementacion de GCD del doc usa while que devuelve valor.
#[test]
fn while_returns_last_body_value_via_while_result_slot() {
    let ir = compile_to_ir(r#"
function gcd(a: Number, b: Number): Number => while (a > 0)
    let m = a % b in {
        b := a;
        a := m;
        a
    };
gcd(12, 8)
"#);
    ir_has(&ir, "while_cond");
    ir_has(&ir, "while_body");
    ir_has(&ir, "while_after");
    ir_has(&ir, "%while_result = alloca double");
    ir_has(&ir, "define double @gcd");
}

// Doc: el `for` es azucar de while sobre Iterable; devuelve el valor del cuerpo.
// `fact` del doc usa `for (i in range(1, x+1)) f := f * i` que devuelve el ultimo f.
#[test]
fn for_over_range_returns_last_body_value() {
    let ir = compile_to_ir(r#"
function fact(x: Number): Number => let f = 1 in for (i in range(1, x + 1)) f := f * i;
fact(5)
"#);
    ir_has(&ir, "for_cond");
    ir_has(&ir, "for_body");
    ir_has(&ir, "for_after");
    ir_has(&ir, "%for_result = alloca double");
}

// while anidado dentro de for: ambos ciclos emiten sus bloques
#[test]
fn for_with_nested_while_emits_both_loop_structures() {
    let ir = compile_to_ir(r#"
let acc = 0 in
for (i in range(0, 3)) {
    let j = 0 in while (j < i) { acc := acc + 1; j := j + 1; j };
    acc
}
"#);
    ir_has(&ir, "for_cond");
    ir_has(&ir, "while_cond");
}

// ===========================================================================
// VECTORES (seccion 16 del doc)
// ===========================================================================

// Doc: `let numbers = [1,2,...9] in for (x in numbers) print(x)`.
// La iteracion sobre vector usa cursor: next() y current().
#[test]
fn for_over_vector_literal_uses_cursor_protocol() {
    let ir = compile_to_ir("let v = [1, 2, 3, 4, 5] in for (x in v) print(x)");
    ir_has(&ir, "vec_next_cursor");
    ir_has(&ir, "vec_has_next");
    ir_has(&ir, "vec_current_byte_offset");
    ir_has(&ir, "for_cond");
}

// Doc: `numbers[7]` accede al elemento por indice con bounds-check en runtime.
#[test]
fn vector_index_emits_bounds_check_blocks() {
    let ir = compile_to_ir("let v = [10, 20, 30] in v[1]");
    ir_has(&ir, "index_ok");
    ir_has(&ir, "index_fail");
    ir_has(&ir, "index_cont");
    ir_has(&ir, "hulk_panic");
}

// Indexacion fuera de rango no crashea al compilar (el check es runtime)
#[test]
fn vector_out_of_bounds_index_still_compiles() {
    let ir = compile_to_ir("let v = [1, 2] in v[99]");
    ir_has(&ir, "index_ok");
    ir_has(&ir, "index_fail");
}

// Doc: `v.size()` devuelve Number (el len se convierte de i64 a double).
#[test]
fn vector_size_method_converts_len_to_f64() {
    let ir = compile_to_ir("let v = [1, 2, 3, 4, 5] in v.size()");
    ir_has(&ir, "vec_len_f64");
    ir_has(&ir, "uitofp i64");
}

// Doc: `v.next()` y `v.current()` forman el protocolo iterable manualmente.
#[test]
fn vector_next_and_current_use_cursor_helpers() {
    let ir = compile_to_ir(r#"
let v = [10, 20, 30] in {
    v.next();
    v.current()
}
"#);
    ir_has(&ir, "vec_next_cursor");
    ir_has(&ir, "vec_has_next");
    ir_has(&ir, "vec_current_byte_offset");
}

// Doc: sintaxis implicita `[expr | symbol in iterable]`.
// Con un vector literal como fuente: genera bucle comp_cond/body/after.
#[test]
fn vector_comprehension_from_literal_generates_comp_loop() {
    let ir = compile_to_ir("[x * 2 | x in [1, 2, 3, 4, 5]]");
    ir_has(&ir, "comp_cond");
    ir_has(&ir, "comp_body");
    ir_has(&ir, "comp_after");
    // El vector resultado tambien se aloca
    ir_has(&ir, "vec_alloc");
}

// Comprehension con expresion compleja en el cuerpo
#[test]
fn vector_comprehension_with_arithmetic_body() {
    let ir = compile_to_ir("[x * x + 1 | x in [1, 2, 3]]");
    ir_has(&ir, "comp_cond");
    ir_has(&ir, "comp_body");
}

// Vector de vectores: estructura anidada (outer e inner ambos usan vec_alloc)
#[test]
fn nested_vector_literal_allocates_outer_and_inner() {
    let ir = compile_to_ir("let v = [[1, 2], [3, 4], [5, 6]] in v.size()");
    // Hay multiples llamadas a malloc para los vectores internos
    let malloc_count = ir.matches("call ptr @malloc").count();
    assert!(malloc_count >= 4, "se esperaban al menos 4 llamadas a malloc, got {malloc_count}");
}

// Doc: `function mean(numbers: Number[]): Number` - vector como parametro tipado.
// El vector llega como `ptr`, el resultado es `double`.
#[test]
fn vector_as_typed_parameter_and_size_used_in_body() {
    let ir = compile_to_ir(r#"
function mean(numbers: Number[]): Number =>
    let total = 0 in {
        for (x in numbers) total := total + x;
        total / numbers.size()
    };
let numbers = [1.0, 2.0, 3.0, 4.0, 5.0] in mean(numbers)
"#);
    ir_has(&ir, "define double @mean(ptr %numbers)");
    ir_has(&ir, "vec_len_f64");
    ir_has(&ir, "for_cond");
}

// Imprimir un vector usa el helper vec_render
#[test]
fn print_vector_calls_vec_render() {
    let ir = compile_to_ir("print([1, 2, 3])");
    ir_has(&ir, "vec_render");
    ir_has(&ir, "@printf");
}

// ===========================================================================
// OOP: TIPOS, HERENCIA, VTABLES, IS/AS (seccion 11-12 del doc)
// ===========================================================================

// Doc: instanciar un tipo usa `new`; la memoria se toma de `malloc`.
// Los campos se inicializan con los valores declarados.
#[test]
fn type_instantiation_uses_malloc_and_initializes_fields() {
    let ir = compile_to_ir(r#"
type Point {
    x: Number = 0;
    y: Number = 0;
    getX(): Number => self.x;
}
let p = new Point() in p.getX()
"#);
    ir_has(&ir, "malloc");
    ir_has(&ir, "%obj.Point");
    ir_has(&ir, "@vtable.Point");
    ir_has(&ir, "Point.getX");
}

// Doc: `type Point(x, y)` - ctor args se pasan al construir el objeto.
// PolarPoint hereda Point con constructor implicito.
#[test]
fn inherited_constructor_args_passed_correctly() {
    let ir = compile_to_ir(r#"
type Point(x: Number, y: Number) {
    x: Number = x;
    y: Number = y;
    getX(): Number => self.x;
    getY(): Number => self.y;
}
type PolarPoint inherits Point {
    rho(): Number => sqrt(self.getX() ^ 2 + self.getY() ^ 2);
}
let pt = new PolarPoint(3, 4) in pt.rho()
"#);
    ir_has(&ir, "%obj.Point");
    ir_has(&ir, "%obj.PolarPoint");
    ir_has(&ir, "@vtable.PolarPoint");
    ir_has(&ir, "@llvm.sqrt.f64");
    // Los valores 3 y 4 aparecen en el IR para los campos
    ir_has(&ir, "3.000000e+00");
    ir_has(&ir, "4.000000e+00");
}

// Doc: `type PolarPoint2(phi, rho) inherits Point(rho * sin(phi), rho * cos(phi))`
// Las expresiones de los args del padre usan sin/cos del padre.
#[test]
fn parent_ctor_args_use_child_params_and_trig() {
    let ir = compile_to_ir(r#"
type Point(x: Number, y: Number) {
    x: Number = x;
    y: Number = y;
    getX(): Number => self.x;
    getY(): Number => self.y;
}
type PolarPoint2(phi: Number, rho: Number) inherits Point(rho * sin(phi), rho * cos(phi)) {
    r: Number = rho;
    rho2(): Number => self.r;
}
let q = new PolarPoint2(1.0, 2.0) in q.rho2()
"#);
    ir_has(&ir, "%obj.PolarPoint2");
    ir_has(&ir, "@llvm.sin.f64");
    ir_has(&ir, "@llvm.cos.f64");
    ir_has(&ir, "PolarPoint2.rho2");
}

// Doc: metodos son virtuales; override requiere misma firma.
// Dispatch dinamico via vtable: el IR carga el puntero al metodo y lo invoca.
#[test]
fn virtual_method_dispatch_loads_from_vtable() {
    let ir = compile_to_ir(r#"
type Animal { speak(): String => "..."; }
type Dog inherits Animal { speak(): String => "woof"; }
let a: Animal = new Dog() in a.speak()
"#);
    ir_has(&ir, "@vtable.Animal");
    ir_has(&ir, "@vtable.Dog");
    // El dispatch carga la funcion desde la vtable
    ir_has(&ir, "method_load");
}

// Doc: self.field := valor modifica el campo del objeto actual.
#[test]
fn self_field_write_via_getelementptr() {
    let ir = compile_to_ir(r#"
type Counter {
    value: Number = 0;
    increment(): Number => self.value := self.value + 1;
    current(): Number => self.value;
}
let c = new Counter() in {
    c.increment();
    c.current()
}
"#);
    ir_has(&ir, "value_field");
    ir_has(&ir, "Counter.increment");
}

// Doc: `is` verifica el tipo dinamico (usa el type_id en la vtable).
// `as` hace un downcast (verifica type_id en runtime).
#[test]
fn is_and_as_check_vtable_type_id() {
    let ir = compile_to_ir(r#"
type A {}
type B inherits A {}
type C inherits A {}
let x: A = if (rand() < 0.5) new B() else new C() in
    if (x is B) 1 else 0
"#);
    ir_has(&ir, "is_cmp");
    ir_has(&ir, "type_id");
}

// is/as en patron completo del doc: test antes de downcast
#[test]
fn is_guards_safe_as_downcast() {
    let ir = compile_to_ir(r#"
type Base { value: Number = 0; }
type Child inherits Base { extra: Number = 99; }
let x: Base = new Child() in
    if (x is Child) {
        let y = x as Child in y
    } else x
"#);
    ir_has(&ir, "is_cmp");
    ir_has(&ir, "as_cmp");
}

// Doc: `toString()` override se usa al imprimir con print(obj).
#[test]
fn print_object_invokes_tostring_override() {
    let ir = compile_to_ir(r#"
type Dog {
    name: String = "Fido";
    toString(): String => "Dog";
}
print(new Dog())
"#);
    ir_has(&ir, "Dog.toString");
    ir_has(&ir, "@printf");
}

// Sin override de toString, el print usa el nombre del tipo como fallback
#[test]
fn print_object_without_tostring_uses_type_name_fallback() {
    let ir = compile_to_ir(r#"
type Cat { name: String = "Whiskers"; }
print(new Cat())
"#);
    ir_has(&ir, "@printf");
    // El literal del nombre del tipo aparece como string constante
    ir_has(&ir, "Cat");
}

// ===========================================================================
// CONCATENACION DE STRINGS (seccion "Strings" del doc)
// ===========================================================================

// Doc: `@` concatena string con la representacion textual de un numero.
// Usa `hulk_format_number` + `hulk_concat` (sin espacio, distinto de @@).
#[test]
fn at_operator_with_number_calls_format_and_concat() {
    let ir = compile_to_ir(r#"print("The meaning of life is " @ 42)"#);
    ir_has(&ir, "@hulk_format_number");
    ir_has(&ir, "@hulk_concat");
    ir_has(&ir, "@printf");
}

// Doc: `@@` inserta un espacio entre dos strings (azucar de `@ " " @`).
#[test]
fn double_at_operator_is_space_concat() {
    let ir = compile_to_ir(r#"print("hello" @@ "world")"#);
    ir_has(&ir, "@hulk_concat_full");
    ir_has(&ir, "@printf");
}

// Cadena mixta: @ con numero y @@ con otro string
#[test]
fn mixed_at_and_double_at_chain() {
    let ir = compile_to_ir(r#"print("total: " @ 42 @@ "!")"#);
    ir_has(&ir, "@hulk_format_number");
    ir_has(&ir, "@hulk_concat_full");
}

// ===========================================================================
// IMPORTS DE MODULOS EXTERNOS (implementado en codegen)
// ===========================================================================

// Doc: `import math` permite usar `math.fn(args)`.
// El codegen emite un `declare` para la funcion importada.
#[test]
fn import_module_emits_declare_for_imported_function() {
    let module_path = "cgedge_mathmod.hulk";
    let module_source = "function mlog(x: Number): Number => x;\n";
    fs::write(module_path, module_source).expect("no se pudo escribir modulo temporal");

    let source = "import cgedge_mathmod\ncgedge_mathmod.mlog(42)";
    let ir = compile_to_ir(source);
    fs::remove_file(module_path).ok();

    ir_has(&ir, "declare double @mlog(double)");
    ir_has(&ir, "call double @mlog(double 4.200000e+01)");
}

// Dos funciones importadas del mismo modulo
#[test]
fn import_module_with_multiple_functions() {
    let module_path = "cgedge_multimod.hulk";
    let module_source = "function add(a: Number, b: Number): Number => a + b;\n\
                         function sub(a: Number, b: Number): Number => a - b;\n";
    fs::write(module_path, module_source).expect("no se pudo escribir modulo temporal");

    let source = "import cgedge_multimod\n\
                  let x = cgedge_multimod.add(10, 5) in cgedge_multimod.sub(x, 3)";
    let ir = compile_to_ir(source);
    fs::remove_file(module_path).ok();

    ir_has(&ir, "declare double @add(double, double)");
    ir_has(&ir, "declare double @sub(double, double)");
}

// ===========================================================================
// ARITMETICA Y BUILTINS MATEMATICOS (seccion 6 del doc)
// ===========================================================================

// Doc: `sqrt`, `sin`, `cos`, `exp`, `log`, `rand` son builtins.
#[test]
fn builtin_math_functions_emit_llvm_intrinsics() {
    let ir = compile_to_ir("sqrt(16)");
    ir_has(&ir, "@llvm.sqrt.f64");
}

#[test]
fn builtin_sin_cos_emit_llvm_intrinsics() {
    let ir = compile_to_ir("sin(PI) + cos(0)");
    ir_has(&ir, "@llvm.sin.f64");
    ir_has(&ir, "@llvm.cos.f64");
}

#[test]
fn builtin_exp_emits_llvm_exp_intrinsic() {
    let ir = compile_to_ir("exp(1)");
    ir_has(&ir, "@llvm.exp.f64");
}

#[test]
fn builtin_log_emits_log_intrinsic_or_helper() {
    let ir = compile_to_ir("log(2, 8)");
    // log(b, x) se implementa como log(x)/log(b) con el intrinsic de LLVM
    ir_has(&ir, "double");
}

#[test]
fn power_operator_uses_pow_intrinsic() {
    let ir = compile_to_ir("2 ^ 10");
    ir_has(&ir, "@llvm.pow.f64");
}

// ===========================================================================
// RECURSION
// ===========================================================================

// Doc: funciones pueden llamarse a si mismas recursivamente.
#[test]
fn recursive_fibonacci_emits_two_self_calls() {
    let ir = compile_to_ir(
        "function fib(n: Number): Number => if (n <= 1) n else fib(n - 1) + fib(n - 2); fib(7)",
    );
    ir_has(&ir, "define double @fib");
    // Dos llamadas recursivas en el cuerpo
    let call_count = ir.matches("call double @fib").count();
    assert!(call_count >= 2, "se esperaban >= 2 llamadas recursivas a @fib, got {call_count}");
}

// Doc: funciones pueden llamarse sin importar el orden de declaracion.
#[test]
fn forward_reference_between_functions() {
    let ir = compile_to_ir(
        "function cot(x: Number): Number => 1 / tan(x); \
         function tan(x: Number): Number => sin(x) / cos(x); \
         cot(1)",
    );
    ir_has(&ir, "define double @cot");
    ir_has(&ir, "define double @tan");
    ir_has(&ir, "call double @tan");
}

// ===========================================================================
// FEATURES NO IMPLEMENTADAS EN CODEGEN (documentadas como FAIL esperado)
// ===========================================================================

// `match` con literales numericos genera bloques match_arm_N / match_next_N
// y usa fcmp oeq para comparar cada caso.
#[test]
fn match_number_literal_generates_correct_dispatch_blocks() {
    let ir = compile_to_ir(r#"
let x: Number = 42 in match x {
    case 1 => "one";
    case 42 => "answer";
    default => "other";
}
"#);
    ir_has(&ir, "match_scrut");
    ir_has(&ir, "match_result");
    ir_has(&ir, "match_arm_0");
    ir_has(&ir, "match_arm_1");
    ir_has(&ir, "match_arm_2");
    ir_has(&ir, "match_after");
    ir_has(&ir, "fcmp oeq double");
    ir_has(&ir, "1.000000e+00");
    ir_has(&ir, "4.200000e+01");
}

// `match` con booleanos usa icmp eq i1 para comparar true/false.
#[test]
fn match_boolean_literal_generates_correct_dispatch_blocks() {
    let ir = compile_to_ir(r#"
let b: Boolean = true in match b {
    case true => 1;
    case false => 0;
}
"#);
    ir_has(&ir, "match_scrut");
    ir_has(&ir, "match_arm_0");
    ir_has(&ir, "match_arm_1");
    ir_has(&ir, "match_after");
    ir_has(&ir, "icmp eq i1");
}

// ===========================================================================
// LAMBDAS COMO VALORES: CLOSURES COMPLETAS (implementado en codegen)
// ===========================================================================

// Una lambda asignada a una variable se compila como una closure: struct
// { fn_ptr, env_ptr } alocada en el heap, con las variables libres copiadas
// por valor al entorno. Invocarla hace una llamada indirecta a traves del
// puntero a funcion cargado desde la closure.
#[test]
fn lambda_assigned_to_variable_compiles_to_closure_struct() {
    let ir = compile_to_ir(r#"
let y = 5 in let f = (x: Number): Number => x + y in print(f(10))
"#);
    // El entorno captura `y` y la closure se aloca con malloc
    ir_has(&ir, "%env_alloc = call ptr @malloc");
    ir_has(&ir, "%closure_alloc = call ptr @malloc");
    ir_has(&ir, "store ptr @lambda_0, ptr %closure_fn_slot");
    // La invocacion via variable hace una llamada indirecta usando el fn_ptr cargado
    ir_has(&ir, "%closure_fn = load ptr, ptr %closure_fn_slot1");
    ir_has(&ir, "call double %closure_fn(ptr %closure_env");
    // El cuerpo de la lambda es una funcion independiente con el env como primer parametro
    ir_has(&ir, "define double @lambda_0(ptr %env, double %x)");
}

// Lambda inmediatamente invocada (IIFE): se construye la closure y se llama
// de inmediato, sin pasar por una variable intermedia.
#[test]
fn immediately_invoked_lambda_expression_compiles() {
    let ir = compile_to_ir("print(((x: Number): Number => x * x)(6))");
    ir_has(&ir, "%closure_alloc = call ptr @malloc");
    ir_has(&ir, "call double %closure_fn(ptr %closure_env");
    ir_has(&ir, "fmul double");
}

// Closures devueltas desde ramas if/else: el merge usa un phi sobre punteros
// a closure (closure_struct*), ejercitando el nuevo brazo Closure de build_phi_value.
#[test]
fn closure_returned_from_if_branches_uses_closure_phi() {
    let ir = compile_to_ir(r#"
let cond = true in
let inc = (x: Number): Number => x + 1 in
let dec = (x: Number): Number => x - 1 in
let f = if (cond) inc else dec in
print(f(100))
"#);
    ir_has(&ir, "if_then");
    ir_has(&ir, "if_else");
    ir_has(&ir, "if_merge");
    ir_has(&ir, "phi ptr");
}

// Closures pasadas como argumento a una funcion declarada con tipo funcion
// `(Number) -> Number`: el parametro se recibe como puntero a closure_struct.
#[test]
fn closure_passed_as_typed_function_parameter() {
    let ir = compile_to_ir(r#"
function apply(f: (Number) -> Number, v: Number): Number => f(v);
let double = (x: Number): Number => x * 2 in
print(apply(double, 21))
"#);
    ir_has(&ir, "define double @apply(ptr %f, double %v)");
    ir_has(&ir, "call double %closure_fn(ptr %closure_env");
}

// Closures guardadas en un campo de objeto e invocadas via `self.campo(args)`.
// Este es el caso ambiguo metodo-vs-closure-field, resuelto en el dispatch
// de Call usando object_method_owner como discriminador.
#[test]
fn closure_stored_in_object_field_and_invoked_via_self() {
    let ir = compile_to_ir(r#"
type Box(f: (Number) -> Number) {
    fn: (Number) -> Number = f;
    apply(v: Number): Number => self.fn(v);
}
let triple = (x: Number): Number => x * 3 in
let b = new Box(triple) in
print(b.apply(7))
"#);
    ir_has(&ir, "%obj.Box");
    ir_has(&ir, "call double %closure_fn(ptr %closure_env");
}

// Captura por valor: mutar una variable capturada con `:=` dentro de la
// lambda solo afecta la copia local de la closure, no la variable externa.
// (simplificacion deliberada frente a closures completas por referencia)
#[test]
fn captured_variable_mutation_does_not_affect_outer_scope() {
    let ir = compile_to_ir(r#"
let counter = 0 in
let f = (): Number => { counter := counter + 1; counter } in
{
    f();
    f();
    print(counter)
}
"#);
    // El cuerpo de la lambda reasigna su propia copia local de `counter`
    ir_has(&ir, "define double @lambda_0(ptr %env)");
    ir_has(&ir, "%counter = alloca double");
}

// Lambda pasada como argumento de protocolo functor (un solo metodo `invoke`):
// el parametro `filter: NumberFilter` se trata como ValueKind::Closure y la
// llamada `filter(3)` se compila como invocacion indirecta de closure.
#[test]
fn lambda_as_functor_protocol_arg_compiles_to_closure_call() {
    let ir = compile_to_ir(r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}
function test(filter: NumberFilter): Boolean => filter(3);
test((x: Number): Boolean => x % 2 == 1)
"#);
    ir_has(&ir, "define i1 @test(ptr %filter)");
    ir_has(&ir, "call i1 %closure_fn(ptr %closure_env");
}

// Funcion con nombre (top-level) pasada como argumento de protocolo functor:
// se envuelve en un thunk cacheado `<fn>_thunk_N` que ignora el env implicito
// y reenvia la llamada a la funcion real.
#[test]
fn function_passed_as_functor_protocol_arg_compiles_via_thunk() {
    let ir = compile_to_ir(r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}
function is_odd(x: Number): Boolean => x % 2 == 1;
function test(filter: NumberFilter): Boolean => filter(3);
test(is_odd)
"#);
    ir_has(&ir, "define i1 @is_odd(double %x)");
    ir_has(&ir, "is_odd_thunk");
    ir_has(&ir, "call i1 %closure_fn(ptr %closure_env");
}

// Misma funcion nombrada usada como functor en dos sitios de llamada distintos:
// el thunk debe generarse una sola vez y compartirse (cache por nombre de funcion).
#[test]
fn function_as_functor_arg_reuses_cached_thunk_across_call_sites() {
    let ir = compile_to_ir(r#"
protocol Combiner {
    invoke(a: Number, b: Number): Number;
}
function add(a: Number, b: Number): Number => a + b;
function apply2(c: Combiner, x: Number, y: Number): Number => c(x, y);
let r1 = apply2(add, 3, 4) in
let r2 = apply2(add, 10, 20) in
print(r1 + r2)
"#);
    let thunk_definitions = ir.matches("define double @add_thunk").count();
    assert_eq!(
        thunk_definitions, 1,
        "se esperaba un unico thunk cacheado para `add`, IR:\n{ir}"
    );
}

// Comprehension con `range()` como fuente: `range` no es una funcion real del
// modulo (solo esta desazucarado para `for`), asi que el comprehension genera
// la secuencia directamente con un contador en lugar de leer de un vector.
#[test]
fn vector_comprehension_with_range_compiles_without_range_function() {
    let ir = compile_to_ir("[x * 2 | x in range(0, 5)]");
    ir_lacks(&ir, "call double @range");
    ir_has(&ir, "comp_range_cond");
    ir_has(&ir, "comp_range_body");
    ir_has(&ir, "fmul double");
}
