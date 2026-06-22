use hulk::code_gen::CodeGenerator;
use hulk::{parse_program, SemanticAnalyzer};
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
        ir.contains("declare double @mlog(double)"),
        "expected imported function declaration, IR:\n{}",
        ir
    );
    assert!(
        ir.contains("call double @mlog(double 4.200000e+01)"),
        "expected imported function call, IR:\n{}",
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