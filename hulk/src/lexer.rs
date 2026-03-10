use logos::Logos;

#[derive(Logos, Debug, PartialEq)]
#[logos(skip r"[ \t\n\f]+")]
#[logos(skip r"//[^\n]*?")]

pub enum Token {
    //keywords
    #[token("let")]
    Let,

    #[token("in")]
    In,

    #[token("if")]
    If,

    #[token("else")]
    Else,

    #[token("elif")]
    Elif,

    #[token("while")]
    While,

    #[token("for")]
    For,

    #[token("function")]
    Function,

    #[token("type")]
    Type,

    #[token("new")]
    New,

    #[token("inherits")]
    Inherits,

    #[token("is")]
    Is,

    #[token("as")]
    As,

    #[token("protocol")]
    Protocol,

    #[token("extends")]
    Extends,

    #[token("def")]
    Def,

    #[token("match")]
    Match,

    #[token("case")]
    Case,

    #[token("default")]
    Default,

    #[token("self")]
    SelfToken,

    #[token("PI")]
    PI,

    #[token("E", priority = 3)]
    E,

    #[token("true")]
    True,

    #[token("false")]
    False,

    #[token("Number")]
    TypeNumber,

    #[token("String")]
    TypeString,

    #[token("Boolean")]
    TypeBoolean,

    #[token("Object")]
    TypeObject,

    #[token("Iterable")]
    TypeIterable,

    //operadores aritmeticos
    #[token("+")]
    Plus,

    #[token("-")]
    Minus,

    #[token("*")]
    Star,

    #[token("/")]
    Slash,

    #[token("^")]
    Caret,

    #[token("%")]
    Mod,

    //operadores de comparacion
    #[token("==")]
    EqualEqual,

    #[token("!=")]
    NotEqual,

    #[token("<=")]
    LessEqual,

    #[token(">=")]
    GreaterEqual,

    #[token("<")]
    Less,

    #[token(">")]
    Greater,

    //operadores logicos
    #[token("&")]
    And,

    #[token("|")]
    Or,

    #[token("!")]
    Not,

    //operadores de asignacion
    #[token("=")]
    Equal,

    #[token(":=")]
    ColonEqual,

    //operadores de concatenacion
    #[token("@@")]
    DoubleAt,

    #[token("@")]
    At,

    //simbolos
    #[token("(")]
    LParen,

    #[token(")")]
    RParen,

    #[token("{")]
    LBrace,

    #[token("}")]
    RBrace,

    #[token("[")]
    LBracket,

    #[token("]")]
    RBracket,

    #[token(",")]
    Comma,

    #[token(".")]
    Dot,

    #[token(":")]
    Colon,

    #[token(";")]
    Semicolon,

    #[token("=>")]
    Arrow,

    #[token("->")]
    ThinArrow,

    #[token("$")]
    Dollar,

    #[token("||")]
    DoublePipe,

    //identificadores y numeros
    #[regex("[a-zA-Z][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),

    #[regex(r"[0-9]+(\.[0-9]+)?", |lex| lex.slice().parse::<f32>().ok())]
    Number(f32),

    //strings
    #[regex(r#""([^"\\]|\\["\\nt])*""#, |lex| lex.slice().to_string())]
    String(String),
}

pub fn lex(source: &str) -> Vec<Token> {
    let mut lexer = Token::lexer(source);
    let mut tokens = Vec::new();
    while let Some(result) = lexer.next() {
        match result {
            Ok(token) => tokens.push(token),
            Err(_) => {
                panic!("Error léxico en: {}", lexer.slice());
            }
        }
    }
    tokens
}
