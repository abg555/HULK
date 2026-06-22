#[cfg(test)]
mod test_parser_debug {
    use hulk::lex_safe;

    #[test]
    fn test_for_even_count_lexer() {
        let input = r#"let evens = 0 in {
    for (i in range(0, 10)) {
        evens := evens + if (i % 2 == 0) 1 else 0;
    };
    if (evens == 5) print("ok") else print("fail");
};"#;

        match lex_safe(input) {
            Ok(tokens) => {
                println!("=== TOKENS ===");
                for (i, token) in tokens.iter().enumerate() {
                    println!("{}: {:?}", i, token);
                }
            }
            Err(e) => {
                panic!("Lexer error: {}", e);
            }
        }
    }
}
