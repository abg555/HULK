use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip "[ \\t\\r\\n\\f]+")]

pub enum Token {
    #[regex(r"//[^\n]*", logos::skip, allow_greedy = true)]
    Comment,

    #[regex(r"/\*(?:[^*]|\*[^/])*\*/", logos::skip, allow_greedy = true)]
    BlockComment,

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

    #[token("base")]
    Base,

    #[token("true")]
    True,

    #[token("false")]
    False,

    // Token sintético para lambdas (no es un keyword, se agrega en post-procesamiento)
    #[doc(hidden)]
    Lambda,

    // Token sintético para distinguir bloques usados como argumento de macro: repeat(10) { ... }
    #[doc(hidden)]
    MacroLBrace,

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
    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        let unquoted = &s[1..s.len()-1];
        let mut result = String::new();
        let mut chars = unquoted.chars();
        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(next_ch) = chars.next() {
                    match next_ch {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        'r' => result.push('\r'),
                        '\\'=> result.push('\\'),
                        '"' => result.push('"'),
                        _ => {
                            result.push('\\');
                            result.push(next_ch);
                        }
                    }
                }
            } else {
                result.push(ch);
            }
        }
        result
    })]
    String(String),
}

fn find_matching_rparen(tokens: &[Token], lparen_idx: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, token) in tokens.iter().enumerate().skip(lparen_idx) {
        match token {
            Token::LParen => depth += 1,
            Token::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_lambda_pattern(tokens: &[Token], rparen_idx: usize) -> bool {
    let mut idx = rparen_idx + 1;
    if idx < tokens.len() && tokens[idx] == Token::Colon {
        idx += 1;
        while idx < tokens.len() {
            match &tokens[idx] {
                Token::Arrow => return true,
                Token::Number(_)
                | Token::String(_)
                | Token::Identifier(_)
                | Token::LBracket
                | Token::RBracket
                | Token::LParen
                | Token::RParen
                | Token::TypeNumber
                | Token::TypeString
                | Token::TypeBoolean => {
                    idx += 1;
                }
                _ => return false,
            }
        }
        false
    } else {
        idx < tokens.len() && tokens[idx] == Token::Arrow
    }
}

pub fn add_lambda_tokens(tokens: Vec<Token>) -> Vec<Token> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        if tokens[i] == Token::LParen {
            // Verificar que el token anterior NO sea una palabra clave o identificador que indica que esto NO es lambda
            let prev_token_is_keyword = i > 0
                && matches!(
                    tokens[i - 1],
                    Token::Case
                        | Token::Match
                        | Token::If
                        | Token::While
                        | Token::For
                        | Token::Function
                        | Token::Def
                        | Token::New
                );

            // Verificar que el token anterior tampoco sea un identificador (evita `function foo(` o `area(`)
            let prev_token_is_identifier = i > 0 && matches!(tokens[i - 1], Token::Identifier(_));

            let is_potential_lambda = !prev_token_is_keyword && !prev_token_is_identifier;

            if is_potential_lambda {
                if let Some(rparen_idx) = find_matching_rparen(&tokens, i) {
                    if is_lambda_pattern(&tokens, rparen_idx) {
                        result.push(Token::Lambda);
                    }
                }
            }
        }
        result.push(tokens[i].clone());
        i += 1;
    }

    result
}

// Paso intermedio: eliminar `||` antes del post-lexer.
// Esto fuerza error de parseo cuando aparezca `||` en el código fuente.
pub fn remove_double_pipe_tokens(tokens: Vec<Token>) -> Vec<Token> {
    tokens
        .into_iter()
        .filter(|token| !matches!(token, Token::DoublePipe))
        .collect()
}

fn find_matching_rbracket(tokens: &[Token], lbracket_idx: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, token) in tokens.iter().enumerate().skip(lbracket_idx) {
        match token {
            Token::LBracket => depth += 1,
            Token::RBracket => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn fix_list_comprehension_pipe(tokens: Vec<Token>) -> Vec<Token> {
    let mut result = tokens.clone();
    let mut i = 0;

    while i < result.len() {
        if result[i] == Token::LBracket {
            if let Some(rbracket_idx) = find_matching_rbracket(&result, i) {
                // Buscar un patrón: Expr | Identifier In Expr dentro de [ ... ]
                for j in (i + 1)..rbracket_idx {
                    if result[j] == Token::Or {
                        // Verificar si después hay Identifier seguido de In
                        if j + 2 < rbracket_idx
                            && matches!(&result[j + 1], Token::Identifier(_))
                            && result[j + 2] == Token::In
                        {
                            // Verificar que antes del | no hay un operador binario
                            // que indicaría que se espera otro operando
                            let prev_is_binary_op = j > 0
                                && matches!(
                                    result[j - 1],
                                    Token::Plus
                                        | Token::Minus
                                        | Token::Star
                                        | Token::Slash
                                        | Token::Caret
                                        | Token::Mod
                                        | Token::And
                                        | Token::EqualEqual
                                        | Token::NotEqual
                                        | Token::Less
                                        | Token::Greater
                                        | Token::LessEqual
                                        | Token::GreaterEqual
                                        | Token::At
                                        | Token::DoubleAt
                                        | Token::Comma
                                        | Token::LParen
                                );

                            if !prev_is_binary_op {
                                // Cambiar | a ||
                                result[j] = Token::DoublePipe;
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    result
}

fn find_matching_paren(tokens: &[Token], lparen_idx: usize) -> Option<usize> {
    let mut depth = 0;
    for (i, token) in tokens.iter().enumerate().skip(lparen_idx) {
        match token {
            Token::LParen => depth += 1,
            Token::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_atomic_expr_end(tokens: &[Token], start: usize) -> Option<usize> {
    if start >= tokens.len() {
        return None;
    }

    match &tokens[start] {
        Token::If => find_inline_if_end(tokens, start),
        Token::LParen => find_matching_paren(tokens, start),
        Token::LBrace => {
            let mut depth = 0;
            for (i, token) in tokens.iter().enumerate().skip(start) {
                match token {
                    Token::LBrace => depth += 1,
                    Token::RBrace => {
                        depth -= 1;
                        if depth == 0 {
                            return Some(i);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        Token::LBracket => find_matching_rbracket(tokens, start),
        Token::Identifier(_)
        | Token::Number(_)
        | Token::String(_)
        | Token::True
        | Token::False
        | Token::SelfToken
        | Token::Base => {
            let mut end = start;
            loop {
                if end + 1 >= tokens.len() {
                    break;
                }

                match tokens[end + 1] {
                    Token::Dot => {
                        if end + 2 < tokens.len() && matches!(tokens[end + 2], Token::Identifier(_))
                        {
                            end += 2;
                        } else {
                            break;
                        }
                    }
                    Token::LParen => {
                        if let Some(rp) = find_matching_paren(tokens, end + 1) {
                            end = rp;
                        } else {
                            break;
                        }
                    }
                    Token::LBracket => {
                        if let Some(rb) = find_matching_rbracket(tokens, end + 1) {
                            end = rb;
                        } else {
                            break;
                        }
                    }
                    _ => break,
                }
            }
            Some(end)
        }
        _ => None,
    }
}

fn find_expression_end(tokens: &[Token], start: usize) -> Option<usize> {
    let mut end = parse_atomic_expr_end(tokens, start)?;

    loop {
        if end + 1 >= tokens.len() {
            return Some(end);
        }

        match &tokens[end + 1] {
            Token::Dot => {
                if end + 2 < tokens.len() && matches!(tokens[end + 2], Token::Identifier(_)) {
                    end += 2;
                } else {
                    return Some(end);
                }
            }
            Token::LParen => {
                if let Some(rp) = find_matching_paren(tokens, end + 1) {
                    end = rp;
                } else {
                    return None;
                }
            }
            Token::LBracket => {
                if let Some(rb) = find_matching_rbracket(tokens, end + 1) {
                    end = rb;
                } else {
                    return None;
                }
            }
            t if is_binary_operator_token(t) => {
                let rhs_start = end + 2;
                if rhs_start >= tokens.len() {
                    return None;
                }
                end = parse_atomic_expr_end(tokens, rhs_start)?;
            }
            Token::Semicolon
            | Token::Comma
            | Token::RParen
            | Token::RBrace
            | Token::RBracket
            | Token::Else
            | Token::Elif => return Some(end),
            _ => return Some(end),
        }
    }
}

fn find_inline_if_end(tokens: &[Token], if_start: usize) -> Option<usize> {
    if if_start >= tokens.len() || tokens[if_start] != Token::If {
        return None;
    }

    if if_start + 1 >= tokens.len() || tokens[if_start + 1] != Token::LParen {
        return None;
    }

    let cond_end = find_matching_paren(tokens, if_start + 1)?;
    let mut cursor = find_expression_end(tokens, cond_end + 1)? + 1;

    while cursor < tokens.len() && tokens[cursor] == Token::Elif {
        if cursor + 1 >= tokens.len() || tokens[cursor + 1] != Token::LParen {
            return None;
        }
        let elif_cond_end = find_matching_paren(tokens, cursor + 1)?;
        cursor = find_expression_end(tokens, elif_cond_end + 1)? + 1;
    }

    if cursor >= tokens.len() || tokens[cursor] != Token::Else {
        return None;
    }

    find_expression_end(tokens, cursor + 1)
}

fn is_binary_operator_token(token: &Token) -> bool {
    matches!(
        token,
        Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::Caret
            | Token::Mod
            | Token::EqualEqual
            | Token::NotEqual
            | Token::Less
            | Token::Greater
            | Token::LessEqual
            | Token::GreaterEqual
            | Token::And
            | Token::Or
            | Token::At
            | Token::DoubleAt
    )
}

pub fn wrap_inline_if_after_binary_ops(tokens: Vec<Token>) -> Vec<Token> {
    let mut result = tokens;
    let mut i = 0;

    while i + 1 < result.len() {
        if is_binary_operator_token(&result[i]) && result[i + 1] == Token::If {
            if let Some(if_end) = find_inline_if_end(&result, i + 1) {
                result.insert(i + 1, Token::LParen);
                result.insert(if_end + 2, Token::RParen);
                i = if_end + 3;
                continue;
            }
        }
        i += 1;
    }

    result
}

pub fn mark_macro_block_calls(tokens: Vec<Token>) -> Vec<Token> {
    let mut result = tokens;

    for i in 0..result.len() {
        let is_identifier = matches!(result.get(i), Some(Token::Identifier(_)));
        let is_lparen_after = matches!(result.get(i + 1), Some(Token::LParen));

        if !is_identifier || !is_lparen_after {
            continue;
        }

        if !is_expr_context_for_macro_call(&result, i) {
            continue;
        }

        if let Some(rparen_idx) = find_matching_rparen(&result, i + 1) {
            if rparen_idx + 1 < result.len() && result[rparen_idx + 1] == Token::LBrace {
                result[rparen_idx + 1] = Token::MacroLBrace;
            }
        }
    }

    result
}

fn is_expr_context_for_macro_call(tokens: &[Token], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }

    matches!(
        tokens[idx - 1],
        Token::Semicolon
            | Token::In
            | Token::Comma
            | Token::LParen
            | Token::Else
            | Token::Arrow
            | Token::ColonEqual
            | Token::Equal
    )
}
