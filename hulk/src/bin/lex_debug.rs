use std::fs;
use hulk::lex_safe;

fn main() {
    let s = fs::read_to_string("src/bin/prueba.hulk").expect("read");
    match hulk::parse_program(&s) {
        Ok(_) => println!("Parse OK"),
        Err(diags) => {
            println!("Parse diagnostics:");
            for d in diags {
                println!("- {} ({}..{})", d.message, d.span.start, d.span.end);
                let snippet = &s[d.span.start..d.span.end.min(s.len())];
                println!("  snippet: {:?}", snippet);
            }
        }
    }
}
