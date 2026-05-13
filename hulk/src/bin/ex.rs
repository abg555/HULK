use inkwell::context::Context;

use hulk::code_gen::CodeGenerator;
use hulk::{parse_program, SemanticAnalyzer};

fn main() {
   let input = "function add(x: Number, y: Number): Number => x + y;

add(2, 3)";

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

    let context = Context::create();
    let mut codegen = CodeGenerator::new(&context, "hulk_test");

    if let Err(message) = codegen.codegen_program(&program, &analysis) {
        eprintln!("Error en codegen: {}", message);
        return;
    }

    codegen.module().print_to_stderr();
}