mod ast;
mod code_gen;
mod diagnostics;
mod lexer;
mod node_ids;
mod preprocessor;
mod semantic; // Asegúrate de que el módulo sea visible
use semantic::functor_desugar;
use preprocessor::{preprocess_new_array_initializers, remove_double_pipe_tokens, wrap_inline_if_after_binary_ops, add_lambda_tokens, fix_list_comprehension_pipe};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process;
use std::process::Command;

use lalrpop_util::lalrpop_mod;
lalrpop_mod!(pub parser);

use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine,
};

/// =======================
/// PARSE (para imports)
/// =======================
pub fn parse_program(input: &str) -> Result<ast::Program, Vec<diagnostics::Diagnostic>> {
    match lex_safe(input) {
        Ok(tokens) => {
            let lalrpop_tokens: Vec<(usize, lexer::Token, usize)> = tokens
                .into_iter()
                .enumerate()
                .map(|(i, token)| (i, token, i))
                .collect();

            parser::ProgramParser::new()
                .parse(lalrpop_tokens)
                .map_err(|e| {
                    vec![diagnostics::Diagnostic::error(
                        format!("Error de sintaxis en módulo importado: {:?}", e),
                        ast::Span { start: 0, end: 0 },
                    )]
                })
        }
        Err((_pos, msg)) => Err(vec![diagnostics::Diagnostic::error(
            format!("Error léxico en módulo importado: {}", msg),
            ast::Span { start: 0, end: 0 },
        )]),
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Uso: ./hulk <archivo.hulk>");
        process::exit(1);
    }

    let file_path = &args[1];
    let input_path = Path::new(file_path);

    let mut file = fs::File::open(input_path)?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;

    println!("Analizando archivo: {}", file_path);

    // =========================
    // 1. LEXER
    // =========================
    let tokens = match lex_safe(&input) {
        Ok(t) => t,
        Err((pos, msg)) => {
            eprintln!("(1,{}) LEXICAL: {}", pos, msg);
            process::exit(1);
        }
    };

    // =========================
    // 2. PARSER
    // =========================
    let lalrpop_tokens: Vec<(usize, lexer::Token, usize)> = tokens
        .into_iter()
        .enumerate()
        .map(|(i, token)| (i, token, i))
        .collect();

    let program_ast = match parser::ProgramParser::new().parse(lalrpop_tokens) {
        Ok(ast) => {
            println!("[OK] Sintaxis correcta");
            ast
        }
        Err(e) => {
            eprintln!("(1,1) SYNTACTIC: Error de sintaxis: {:?}", e);
            process::exit(2);
        }
    };

    // =========================
    // 3. MACRO EXPANSION
    // =========================
    println!("=== MACRO EXPAND ===");

    let expanded_ast = match semantic::macro_expander::expand_program(program_ast) {
        Ok(ast) => ast,
        Err(diagnostics) => {
            eprintln!("(1,1) SEMANTIC: Error expandiendo macros:");
            for d in diagnostics {
                eprintln!("  - {:?}", d);
            }
            process::exit(3);
        }
    };

    // =========================
    // 4. SEMANTIC
    // =========================
    println!("=== SEMANTIC ===");

    let mut analyzer = semantic::SemanticAnalyzer::new();

    let initial_context = match analyzer.analyze(&expanded_ast) {
        Ok(ctx) => ctx,
        Err(diagnostics) => {
            eprintln!("(1,1) SEMANTIC: Se encontraron errores semánticos:");
            for d in diagnostics {
                eprintln!("  - {:?}", d);
            }
            process::exit(3);
        }
    };

    // ===================================================
    // 4b. DESAZUCARADO
    // ===================================================
    println!("=== DESUGAR ===");
    // Usamos la referencia directa al submódulo que importamos con el 'use'
    let program_desugared = functor_desugar::desugar_program(expanded_ast, &initial_context);

    // ===================================================
    // 4c. RE-ANÁLISIS SEMÁNTICO (Post-Desugar)
    // ===================================================
    let mut post_analyzer = semantic::SemanticAnalyzer::new();
    let semantic_context = match post_analyzer.analyze(&program_desugared) {
        Ok(ctx) => {
            println!("[OK] Semántica correcta");
            ctx
        }
        Err(diagnostics) => {
            eprintln!("(1,1) SEMANTIC (post-desugar): Se encontraron errores:");
            for d in diagnostics {
                eprintln!("  - {:?}", d);
            }
            process::exit(3);
        }
    };

    // =========================
    // 5. CODEGEN LLVM
    // =========================
    println!("=== CODEGEN ===");

    Target::initialize_native(&InitializationConfig::default()).expect("LLVM init failed");

    let context = Context::create();

    // Inicializamos usando el mismo constructor del script que te funciona bien
    let file_stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let mut generator = code_gen::CodeGenerator::with_source_dir(&context, &file_stem, input_path);

    // Pasamos el AST desazucarado en lugar del original
    if let Err(e) = generator.codegen_program(&program_desugared, &semantic_context) {
        eprintln!("CODEGEN ERROR: {}", e);
        process::exit(4);
    }

    // =========================
    // 6. GENERAR OUTPUT REAL (LLVM -> object -> binary)
    // =========================

    let triple = TargetMachine::get_default_triple();
    let target = Target::from_triple(&triple).unwrap();

    let target_machine = target
        .create_target_machine(
            &triple,
            "generic",
            "",
            inkwell::OptimizationLevel::Default,
            RelocMode::Default,
            CodeModel::Default,
        )
        .expect("No se pudo crear TargetMachine");

    let obj_path = "output.o";

    target_machine
        .write_to_file(generator.module(), FileType::Object, Path::new(obj_path))
        .expect("Error generando object file");

    // Compilar runtime.rs si existe para asegurar dependencias de la biblioteca estándar
    let runtime_src = Path::new("runtime.rs");
    let runtime_lib = Path::new("libruntime.a");
    if runtime_src.exists() {
        let _ = Command::new("rustc")
            .args(["--crate-type=staticlib", "runtime.rs", "-o", "libruntime.a"])
            .output();
    }

    let mut clang_args = vec![obj_path];
    if runtime_src.exists() && runtime_lib.exists() {
        clang_args.push("libruntime.a");
    }
    clang_args.extend(["-lm", "-o", "output", "-no-pie"]);

    let status = Command::new("clang")
        .args(&clang_args)
        .status()
        .expect("Error ejecutando clang");

    if !status.success() {
        eprintln!("Linking failed");
        process::exit(4);
    }

    Ok(())
}

// =========================
// LEXER SAFE
// =========================
fn lex_safe(input: &str) -> Result<Vec<lexer::Token>, (usize, String)> {
    use logos::Logos;

    // Preprocess source: convert `{...}` patterns to appropriate brackets
    let preprocessed = preprocess_new_array_initializers(input);

    // Tokenize
    let mut tokens = Vec::new();
    let mut lexer = lexer::Token::lexer(&preprocessed);

    while let Some(result) = lexer.next() {
        match result {
            Ok(tok) => tokens.push(tok),
            Err(_) => {
                return Err((
                    lexer.span().start,
                    format!("Token no reconocido: {:?}", lexer.slice()),
                ));
            }
        }
    }

    // Post-lexing token transformations
    let tokens = remove_double_pipe_tokens(tokens);
    let tokens = wrap_inline_if_after_binary_ops(tokens);
    let tokens = add_lambda_tokens(tokens);
    let tokens = fix_list_comprehension_pipe(tokens);

    Ok(tokens)
}
