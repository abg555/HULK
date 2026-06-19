use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip "[ \\t\\r\\n\\f]+")]

pub enum Token {
    #[regex(r"//[^\n]*", logos::skip, allow_greedy = true)]
    Comment,

    #[regex(r"/\*(?:[^*]|\*[^/])*\*/", logos::skip, allow_greedy = true)]
    BlockComment,

    //keywords
    #[token("import")]
    Import,

    #[token("export")]
    Export,

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

    #[token("interface")]
    Interface,

    #[token("extends")]
    Extends,

    #[token("def")]
    Def,

    #[token("define")]
    Define,

    #[token("match")]
    Match,

    #[token("case")]
    Case,

    #[token("default")]
    Default,

    #[token("self")]
    SelfToken,

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

    #[regex(r"[0-9]+(\.[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    Number(f64),

    //strings
    #[regex(r#""([^"\\]|\\[ntr"\\])*""#, |lex| {
    let s = lex.slice();
    let unquoted = &s[1..s.len()-1];

    let mut result = String::new();
    let mut chars = unquoted.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next().unwrap() {
                'n' => result.push('\n'),
                't' => result.push('\t'),
                'r' => result.push('\r'),
                '\\' => result.push('\\'),
                '"' => result.push('"'),
                _ => unreachable!(),
            }
        } else {
            result.push(ch);
        }
    }

    result
})]
    String(String),
}

