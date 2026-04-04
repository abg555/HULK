mod ast;
mod lexer;

use std::fs;
use std::io::{self, Read};

fn main() -> io::Result<()> {
    // Leer el archivo de ejemplos
    let mut file = fs::File::open("examples.hulk")?;
    let mut input = String::new();
    file.read_to_string(&mut input)?;

    println!("Analizando archivo: examples.hulk");
    println!("Longitud del archivo: {} caracteres\n", input.len());

    // Tokenizar
    println!("=== LEXING ===");
    match lex_safe(&input) {
        Ok(tokens) => {
            println!("[OK] Tokens generados: {}\n", tokens.len());

            // Mostrar los primeros 100 tokens como ejemplo
            println!("Primeros 100 tokens:");
            for (i, token) in tokens.iter().take(100).enumerate() {
                println!("{:3}: {:?}", i, token);
            }

            println!("\n... (total de {} tokens)\n", tokens.len());

            // Buscar tokens Lambda
            let lambda_count = tokens
                .iter()
                .filter(|t| matches!(t, lexer::Token::Lambda))
                .count();
            println!("[INFO] Tokens Lambda encontrados: {}", lambda_count);

            if lambda_count > 0 {
                println!("\nPosiciones de tokens Lambda:");
                for (i, token) in tokens.iter().enumerate() {
                    if matches!(token, lexer::Token::Lambda) {
                        // Mostrar 5 tokens antes y después
                        let start = if i >= 5 { i - 5 } else { 0 };
                        let end = std::cmp::min(i + 6, tokens.len());
                        println!("\nLambda en posición {}:", i);
                        for j in start..end {
                            let marker = if j == i { " >>> " } else { "     " };
                            println!("{}{:3}: {:?}", marker, j, tokens[j]);
                        }
                    }
                }
            }

            println!("\n[OK] El lexer puede procesar correctamente todos los ejemplos");
        }
        Err(e) => {
            println!("[ERROR] Fallo en lexing: {}", e);
        }
    }

    Ok(())
}

fn lex_safe(input: &str) -> Result<Vec<lexer::Token>, String> {
    use logos::Logos;

    let mut tokens = Vec::new();
    let mut lexer = lexer::Token::lexer(input);

    while let Some(result) = lexer.next() {
        match result {
            Ok(token) => {
                tokens.push(token);
            }
            Err(_) => {
                let slice = lexer.slice();
                return Err(format!("Token no reconocido: {:?}", slice));
            }
        }
    }

    // Post-procesar tokens para agregar marcadores de lambda
    let tokens = lexer::add_lambda_tokens(tokens);

    // Post-procesar tokens para convertir | a || en list comprehensions
    let tokens = lexer::fix_list_comprehension_pipe(tokens);

    // Post-procesar tokens para envolver if inline tras + o -
    let tokens = lexer::wrap_inline_if_after_add_sub(tokens);

    // Post-procesar tokens para marcar bloques usados en llamadas de macro: id(...) { ... }
    let tokens = lexer::mark_macro_block_calls(tokens);

    Ok(tokens)
}
