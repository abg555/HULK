use inkwell::context::Context;

use hulk::code_gen::CodeGenerator;
use hulk::{parse_program, SemanticAnalyzer};

fn main() {
    let input = r#"
let v = [1, 2, 3] in {
    v.next();
    for (x in v) x;
}
"#;

    let program = match parse_program(input) {
        Ok(program) => program,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{}", diagnostic.message);
            }
            return;
        }
    };

    let analysis = match SemanticAnalyzer::new().analyze(&program) {
        Ok(analysis) => analysis,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{}", diagnostic.message);
            }
            return;
        }
    };

    // Dump inferred types for debugging codegen errors
    eprintln!("Inferred types:");
    for (node, typ) in &analysis.inferred_types {
        eprintln!("  {:?} -> {}", node, typ);
    }

    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "hulk_test");

    if let Err(message) = codegen.codegen_program(&program, &analysis) {
        eprintln!("Error en codegen: {}", message);
        return;
    }

    codegen.module().print_to_stderr();
}