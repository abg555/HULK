use hulk::code_gen::CodeGenerator;
use hulk::{desugar_functors, parse_program, SemanticAnalyzer};
use inkwell::context::Context;
use std::fs;

#[derive(Debug)]
struct CodegenCase {
    name: String,
    source: String,
    expectations: Vec<String>,
    expected_error: Option<String>,
}

fn load_cases() -> Vec<CodegenCase> {
    let mut cases = Vec::new();
    let mut current: Option<CodegenCase> = None;

    for line in include_str!("codegen_cases.hulk").lines() {
        let trimmed = line.trim();
        let lowered = trimmed.to_ascii_lowercase();

        if lowered.starts_with("// example:") || lowered.starts_with("// ejemplo:") {
            if let Some(case) = current.take() {
                if !case.source.trim().is_empty() {
                    cases.push(case);
                }
            }

            let name = trimmed
                .split_once(':')
                .map(|(_, rest)| rest.trim())
                .unwrap_or("unnamed");

            current = Some(CodegenCase {
                name: name.to_string(),
                source: String::new(),
                expectations: Vec::new(),
                expected_error: None,
            });
            continue;
        }

        let Some(case) = current.as_mut() else {
            continue;
        };

        if trimmed.starts_with("// expect-error:") {
            case.expected_error = Some(trimmed["// expect-error:".len()..].trim().to_string());
            continue;
        }

        if trimmed.starts_with("// expect:") {
            case.expectations
                .push(trimmed["// expect:".len()..].trim().to_string());
            continue;
        }

        if trimmed.starts_with("//") {
            continue;
        }

        if line.is_empty() {
            if !case.source.is_empty() {
                case.source.push('\n');
            }
            continue;
        }

        case.source.push_str(line);
        case.source.push('\n');
    }

    if let Some(case) = current.take() {
        if !case.source.trim().is_empty() {
            cases.push(case);
        }
    }

    cases
}

fn compile_to_ir(source: &str) -> String {
    let program = parse_program(source).expect("expected parse success");
    let analysis = SemanticAnalyzer::new()
        .analyze(&program)
        .expect("expected semantic success");

    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "codegen_test");

    codegen
        .codegen_program(&program, &analysis)
        .expect("expected codegen success");

    codegen.module().print_to_string().to_string()
}

/// Like `compile_to_ir` but also runs the functor desugar pass, which is
/// needed for programs that use lambdas or function-typed parameters.
fn compile_to_ir_with_functors(source: &str) -> String {
    let program = desugar_functors(source).expect("expected desugar success");
    let analysis = SemanticAnalyzer::new()
        .analyze(&program)
        .expect("expected semantic success after desugar");

    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "codegen_test");

    codegen
        .codegen_program(&program, &analysis)
        .expect("expected codegen success");

    codegen.module().print_to_string().to_string()
}

fn codegen_error(source: &str) -> String {
    let program = parse_program(source).expect("expected parse success");
    let analysis = SemanticAnalyzer::new()
        .analyze(&program)
        .expect("expected semantic success");

    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "codegen_test");

    codegen
        .codegen_program(&program, &analysis)
        .expect_err("expected codegen error")
}

#[test]
fn codegen_cases_are_written_in_hulk() {
    let cases = load_cases();
    assert!(!cases.is_empty(), "expected at least one codegen case");

    for case in cases {
        match &case.expected_error {
            Some(expected_error) => {
                let err = codegen_error(&case.source);
                assert!(
                    err.contains(expected_error),
                    "case {} expected error {:?}, got: {}",
                    case.name,
                    expected_error,
                    err
                );
            }
            None => {
                let ir = compile_to_ir(&case.source);

                for expected in &case.expectations {
                    assert!(
                        ir.contains(expected),
                        "case {} expected IR to contain {:?}, but it did not. IR:\n{}",
                        case.name,
                        expected,
                        ir
                    );
                }
            }
        }
    }
}

#[test]
fn codegen_supports_namespace_import_function_calls() {
    let module_path = "codegen_math.hulk";
    let module_source = r#"
function mlog(x: Number): Number => x;
"#;
    fs::write(module_path, module_source).expect("expected temp module creation");

    let source = r#"
import codegen_math

codegen_math.mlog(42)
"#;

    let ir = compile_to_ir(source);
    fs::remove_file(module_path).ok();

    assert!(
        ir.contains("define double @mlog("),
        "expected imported function definition, IR:\n{}",
        ir
    );
    assert!(
        ir.contains("call double @mlog(double 4.200000e+01)"),
        "expected imported function call, IR:\n{}",
        ir
    );
}

#[test]
fn codegen_lowers_boolean_match_to_branches() {
    let source = r#"
let x: Boolean = true in match x {
  case true  => 1;
  case false => 0;
}
"#;
    let ir = compile_to_ir(source);
    assert!(
        ir.contains("match_case_0") && ir.contains("match_case_1") && ir.contains("match_merge"),
        "expected match branch labels, IR:\n{}",
        ir
    );
}

#[test]
fn codegen_lowers_number_match_with_default() {
    let source = r#"
let n: Number = 3 in match n {
  case 1 => "one";
  case 2 => "two";
  default => "other";
}
"#;
    let ir = compile_to_ir(source);
    assert!(
        ir.contains("match_merge"),
        "expected match_merge block, IR:\n{}",
        ir
    );
    // default case must jump unconditionally (no branch condition for it)
    assert!(
        ir.contains("match_case_2"),
        "expected default case block, IR:\n{}",
        ir
    );
}

#[test]
fn codegen_lowers_type_pattern_match() {
    let source = r#"
type Animal {
    sound(): String => "generic";
}
type Dog inherits Animal {
    sound(): String => "woof";
}
type Cat inherits Animal {
    sound(): String => "meow";
}

function check(a: Animal): Number => match a {
  case d: Dog => 1;
  default     => 0;
};
check(new Dog())
"#;
    let ir = compile_to_ir(source);
    assert!(
        ir.contains("match_merge"),
        "expected match_merge block for type-pattern match, IR:\n{}",
        ir
    );
}

#[test]
fn codegen_lowers_lambda_to_anonymous_function() {
    // functor_desugar wraps the lambda in a _FunctorWrapper type with an
    // `invoke` method; codegen emits a new object + method call.
    let source = r#"
protocol Transformer {
    invoke(x: Number): Number;
}

function apply(f: Transformer, x: Number): Number => f(x);
apply((n: Number): Number => n * 2, 21)
"#;
    let ir = compile_to_ir_with_functors(source);
    assert!(
        ir.contains("_FunctorWrapper") || ir.contains("_lambda"),
        "expected lambda or functor wrapper in IR:\n{}",
        ir
    );
}

#[test]
fn codegen_accepts_macro_expanded_arithmetic_programs() {
    let source = r#"
def twice(x: Number) => x * 2;

twice(21)
"#;

    let ir = compile_to_ir(source);
    assert!(
        ir.contains("ret double 4.200000e+01"),
        "expected successful lowering of expanded macro expression, IR:\n{}",
        ir
    );
}

#[test]
fn codegen_accepts_trailing_block_macro_programs() {
    let source = r#"
def unless(cond: Boolean, *body) => if (!cond) body else 0;

function run(flag: Boolean): Number => unless(flag) {
    42;
};

run(false)
"#;

    let ir = compile_to_ir(source);
    assert!(
        ir.contains("if_then") && ir.contains("if_merge"),
        "expected if lowering from expanded trailing-block macro, IR:\n{}",
        ir
    );
}
#[test]
fn debug_type_mismatch_parse() {
    let source = r#"function add(x: Number, y: Number): Number {
    x + y;
}

{
    add("hello", 5);
};"#;
    let result = hulk::parse_program(source);
    match result {
        Ok(_) => println!("PARSED OK"),
        Err(e) => println!("PARSE ERROR: {:?}", e),
    }
}
