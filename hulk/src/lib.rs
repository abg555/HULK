pub mod ast;
pub mod lexer;

use lalrpop_util::lalrpop_mod;

lalrpop_util::lalrpop_mod!(pub parser);

pub use ast::*;
pub use lexer::*;
pub use parser::ProgramParser;

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

    let tokens = lexer::add_lambda_tokens(tokens);
    let tokens = lexer::fix_list_comprehension_pipe(tokens);
    let tokens = lexer::wrap_inline_if_after_add_sub(tokens);
    let tokens = lexer::mark_macro_block_calls(tokens);

    Ok(tokens)
}
