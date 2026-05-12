pub mod ast;
pub mod code_gen;
pub mod diagnostics;
pub mod lexer;
pub mod node_ids;
pub mod semantic;

use lalrpop_util::lalrpop_mod;

lalrpop_util::lalrpop_mod!(pub parser);

pub use ast::*;
pub use diagnostics::Diagnostic;
pub use lexer::*;
pub use parser::ProgramParser;
pub use semantic::{SemanticAnalysis, SemanticAnalyzer, SemanticContext};
pub use semantic::{macro_expander, symbol_table, types};

pub fn lex_safe(input: &str) -> Result<Vec<lexer::Token>, String> {
    use logos::Logos;

    let mut tokens = Vec::new();
    let mut lexer = lexer::Token::lexer(input);

    while let Some(result) = lexer.next() {
        match result {
            Ok(token) => tokens.push(token),
            Err(_) => {
                let slice = lexer.slice();
                return Err(format!("Token no reconocido: {:?}", slice));
            }
        }
    }

    let tokens = lexer::remove_double_pipe_tokens(tokens);
    let tokens = lexer::add_lambda_tokens(tokens);
    let tokens = lexer::fix_list_comprehension_pipe(tokens);
    let tokens = lexer::wrap_inline_if_after_binary_ops(tokens);
    let tokens = lexer::mark_macro_block_calls(tokens);

    Ok(tokens)
}

pub fn parse_program(input: &str) -> Result<Program, Vec<Diagnostic>> {
    let tokens = lex_safe(input).map_err(|message| {
        vec![Diagnostic::error(
            message,
            Span {
                start: 0,
                end: input.len(),
            },
        )]
    })?;

    let parser = ProgramParser::new();
    let input_tokens = tokens
        .into_iter()
        .enumerate()
        .map(|(i, token)| (i, token, i + 1));

    parser
        .parse(input_tokens)
        .map_err(|error| {
            let diagnostic = match error {
                lalrpop_util::ParseError::InvalidToken { location } => Diagnostic::error(
                    "Token invalido durante el parseo",
                    Span {
                        start: location,
                        end: location,
                    },
                ),
                lalrpop_util::ParseError::UnrecognizedEof { location, .. } => Diagnostic::error(
                    "Fin de archivo inesperado",
                    Span {
                        start: location,
                        end: location,
                    },
                ),
                lalrpop_util::ParseError::UnrecognizedToken {
                    token: (start, _, end),
                    ..
                } => Diagnostic::error("Token inesperado", Span { start, end }),
                lalrpop_util::ParseError::ExtraToken {
                    token: (start, _, end),
                } => Diagnostic::error("Token extra al final", Span { start, end }),
                lalrpop_util::ParseError::User { .. } => Diagnostic::error(
                    "Error interno de parseo",
                    Span {
                        start: 0,
                        end: input.len(),
                    },
                ),
            };
            vec![diagnostic]
        })
        .and_then(semantic::macro_expander::expand_program)
        .map(|mut program| {
            node_ids::assign_program_node_ids(&mut program);
            program
        })
}

pub fn analyze_program(input: &str) -> Result<SemanticContext, Vec<Diagnostic>> {
    let program = parse_program(input)?;
    SemanticAnalyzer::new().analyze(&program)
}
