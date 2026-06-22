//! Casos esquinados del LEXER.
//!
//! Estas pruebas ejercitan `hulk::lex_safe`, que tokeniza la entrada y aplica
//! el post-procesamiento (lambdas, macros, etc.). Se cubren: palabras clave vs
//! identificadores, numeros con/sin decimales, strings con escapes, comentarios
//! de linea y de bloque, operadores multi-caracter y errores lexicos.

use hulk::{lex_safe, Token};

fn lex(input: &str) -> Vec<Token> {
    lex_safe(input).unwrap_or_else(|e| panic!("error lexico inesperado en {input:?}: {e}"))
}

#[test]
fn keywords_are_not_confused_with_identifiers() {
    let tokens = lex("let in if else elif while for function type new inherits");
    assert_eq!(
        tokens,
        vec![
            Token::Let,
            Token::In,
            Token::If,
            Token::Else,
            Token::Elif,
            Token::While,
            Token::For,
            Token::Function,
            Token::Type,
            Token::New,
            Token::Inherits,
        ]
    );
}

#[test]
fn identifier_with_keyword_prefix_is_an_identifier() {
    // "lettuce", "information", "iffy" empiezan con palabras clave pero son ids.
    let tokens = lex("lettuce information iffy forall newish");
    assert_eq!(
        tokens,
        vec![
            Token::Identifier("lettuce".to_string()),
            Token::Identifier("information".to_string()),
            Token::Identifier("iffy".to_string()),
            Token::Identifier("forall".to_string()),
            Token::Identifier("newish".to_string()),
        ]
    );
}

#[test]
fn identifier_allows_digits_and_underscores_in_body() {
    // El regex de identificador exige [a-zA-Z] inicial pero admite digitos y '_'
    // en el cuerpo.
    let tokens = lex("foo_bar1 rejected_by_regex x9");
    assert_eq!(
        tokens,
        vec![
            Token::Identifier("foo_bar1".to_string()),
            Token::Identifier("rejected_by_regex".to_string()),
            Token::Identifier("x9".to_string()),
        ]
    );
}

#[test]
fn leading_underscore_is_a_lex_error() {
    // El '_' inicial no comienza ningun token: error lexico.
    assert!(lex_safe("_is").is_err());
}

#[test]
fn builtin_type_keywords_tokenize_separately() {
    let tokens = lex("Number String Boolean");
    assert_eq!(
        tokens,
        vec![Token::TypeNumber, Token::TypeString, Token::TypeBoolean]
    );
}

#[test]
fn integer_and_decimal_numbers() {
    let tokens = lex("0 42 3.14 100.0");
    assert_eq!(
        tokens,
        vec![
            Token::Number(0.0),
            Token::Number(42.0),
            Token::Number(3.14),
            Token::Number(100.0),
        ]
    );
}

#[test]
fn trailing_dot_number_splits_into_number_and_dot() {
    // El regex de numero es [0-9]+(\.[0-9]+)? : "3." => Number(3) seguido de Dot.
    let tokens = lex("3.");
    assert_eq!(tokens, vec![Token::Number(3.0), Token::Dot]);
}

#[test]
fn leading_dot_number_splits_into_dot_and_number() {
    let tokens = lex(".5");
    assert_eq!(tokens, vec![Token::Dot, Token::Number(5.0)]);
}

#[test]
fn member_access_chain_tokenizes_dots() {
    let tokens = lex("a.b.c");
    assert_eq!(
        tokens,
        vec![
            Token::Identifier("a".to_string()),
            Token::Dot,
            Token::Identifier("b".to_string()),
            Token::Dot,
            Token::Identifier("c".to_string()),
        ]
    );
}

#[test]
fn string_escape_sequences_are_decoded() {
    let tokens = lex(r#""line\nbreak\ttab\"quote\\slash""#);
    assert_eq!(
        tokens,
        vec![Token::String("line\nbreak\ttab\"quote\\slash".to_string())]
    );
}

#[test]
fn unknown_escape_keeps_backslash() {
    // Un escape no reconocido (\q) conserva la barra invertida.
    let tokens = lex(r#""a\qb""#);
    assert_eq!(tokens, vec![Token::String("a\\qb".to_string())]);
}

#[test]
fn empty_string_literal() {
    let tokens = lex(r#""""#);
    assert_eq!(tokens, vec![Token::String(String::new())]);
}

#[test]
fn line_comment_is_skipped_until_newline() {
    let tokens = lex("1 // esto es ignorado hasta el salto\n2");
    assert_eq!(tokens, vec![Token::Number(1.0), Token::Number(2.0)]);
}

#[test]
fn block_comment_is_skipped() {
    let tokens = lex("1 /* comentario\nmultilinea */ 2");
    assert_eq!(tokens, vec![Token::Number(1.0), Token::Number(2.0)]);
}

#[test]
fn block_comment_in_the_middle_of_expression() {
    let tokens = lex("a /* x */ + /* y */ b");
    assert_eq!(
        tokens,
        vec![
            Token::Identifier("a".to_string()),
            Token::Plus,
            Token::Identifier("b".to_string()),
        ]
    );
}

#[test]
fn multi_char_operators_are_greedy() {
    // == debe ganar a = =, := a : =, => a = >, etc.
    let tokens = lex("== != <= >= := => -> @@");
    assert_eq!(
        tokens,
        vec![
            Token::EqualEqual,
            Token::NotEqual,
            Token::LessEqual,
            Token::GreaterEqual,
            Token::ColonEqual,
            Token::Arrow,
            Token::ThinArrow,
            Token::DoubleAt,
        ]
    );
}

#[test]
fn single_char_operators_when_not_part_of_multichar() {
    let tokens = lex("= < > : @ + - * / ^ %");
    assert_eq!(
        tokens,
        vec![
            Token::Equal,
            Token::Less,
            Token::Greater,
            Token::Colon,
            Token::At,
            Token::Plus,
            Token::Minus,
            Token::Star,
            Token::Slash,
            Token::Caret,
            Token::Mod,
        ]
    );
}

#[test]
fn adjacent_operators_without_spaces() {
    // ">=:=" debe partirse como GreaterEqual seguido de ColonEqual.
    let tokens = lex(">=:=");
    assert_eq!(tokens, vec![Token::GreaterEqual, Token::ColonEqual]);
}

#[test]
fn brackets_braces_and_parens() {
    let tokens = lex("([{}])");
    assert_eq!(
        tokens,
        vec![
            Token::LParen,
            Token::LBracket,
            Token::LBrace,
            Token::RBrace,
            Token::RBracket,
            Token::RParen,
        ]
    );
}

#[test]
fn whitespace_variants_are_skipped() {
    // Espacios, tabs, retornos de carro y saltos de linea no producen tokens.
    let tokens = lex("\t1\r\n  2 \t 3\n");
    assert_eq!(
        tokens,
        vec![Token::Number(1.0), Token::Number(2.0), Token::Number(3.0)]
    );
}

#[test]
fn boolean_literals_and_self_and_base_keywords() {
    let tokens = lex("true false self base");
    assert_eq!(
        tokens,
        vec![Token::True, Token::False, Token::SelfToken, Token::Base]
    );
}

#[test]
fn macro_and_module_keywords() {
    let tokens = lex("def import export protocol extends match case default");
    assert_eq!(
        tokens,
        vec![
            Token::Def,
            Token::Import,
            Token::Export,
            Token::Protocol,
            Token::Extends,
            Token::Match,
            Token::Case,
            Token::Default,
        ]
    );
}

#[test]
fn unknown_character_produces_lex_error() {
    // El simbolo '#' no esta en la gramatica lexica.
    let err = lex_safe("1 # 2").expect_err("se esperaba error lexico para '#'");
    assert!(err.contains("Token no reconocido"), "mensaje: {err}");
}

#[test]
fn unterminated_string_is_a_lex_error() {
    let err = lex_safe(r#""sin cerrar"#).expect_err("se esperaba error por string sin cerrar");
    assert!(err.contains("Token no reconocido"), "mensaje: {err}");
}

#[test]
fn empty_input_yields_no_tokens() {
    assert!(lex("").is_empty());
    assert!(lex("   \n\t  ").is_empty());
}

#[test]
fn only_comment_yields_no_tokens() {
    assert!(lex("// solo un comentario").is_empty());
    assert!(lex("/* solo bloque */").is_empty());
}

#[test]
fn dollar_and_at_symbols_for_macros() {
    let tokens = lex("$x @y");
    assert_eq!(
        tokens,
        vec![
            Token::Dollar,
            Token::Identifier("x".to_string()),
            Token::At,
            Token::Identifier("y".to_string()),
        ]
    );
}
