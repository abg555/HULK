mod ast;
mod lexer;

fn main() {
    let input = "let a = 6, b = a * 7 in print(b);";
    println!("Input: {}", input);

    let tokens = lexer::lex(input);
    for token in tokens {
        println!("{:?}", token);
    }
}
