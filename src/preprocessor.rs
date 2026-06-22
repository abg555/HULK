use crate::lexer::Token;

// ==========================================
// HELPER FUNCTIONS FOR PREPROCESSING TOKENS
// ==========================================

pub fn find_matching_rparen(tokens: &[Token], lparen_idx: usize) -> Option<usize> {
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
                for j in (i + 1)..rbracket_idx {
                    if result[j] == Token::Or {
                        if j + 2 < rbracket_idx
                            && matches!(&result[j + 1], Token::Identifier(_))
                            && result[j + 2] == Token::In
                        {
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
        | Token::SelfToken => Some(start),
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

// ==========================================
// SOURCE TEXT PREPROCESSING
// ==========================================

/// Replace `{ ... }` with `( ... )` for `new Type[expr]{...}` (initializer with lambda).
/// Also replace `{ ... }` with `[ ... ]` for array literals like `{1,2,3}` (contains only commas, not semicolons).
fn skip_whitespace(input: &str, mut i: usize) -> usize {
    while let Some(ch) = input[i..].chars().next() {
        if ch.is_whitespace() {
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    i
}

fn find_matching_delimiter(input: &str, mut i: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    while i < input.len() {
        let ch = input[i..].chars().next().unwrap();
        if ch == open {
            depth += 1;
            i += ch.len_utf8();
        } else if ch == close {
            depth -= 1;
            i += ch.len_utf8();
            if depth == 0 {
                return Some(i);
            }
        } else if ch == '"' || ch == '\'' {
            // Skip string/char literals to avoid false delimiter matches.
            let quote = ch;
            i += ch.len_utf8();
            while i < input.len() {
                let c2 = input[i..].chars().next().unwrap();
                i += c2.len_utf8();
                if c2 == quote {
                    break;
                }
                if c2 == '\\' && i < input.len() {
                    i += input[i..].chars().next().unwrap().len_utf8();
                }
            }
        } else {
            i += ch.len_utf8();
        }
    }
    None
}

fn prev_non_whitespace_char(input: &str, mut i: usize) -> Option<char> {
    while i > 0 {
        i -= 1;
        let ch = input[i..].chars().next().unwrap();
        if ch.is_whitespace() {
            continue;
        }
        return Some(ch);
    }
    None
}

fn is_array_comprehension(input: &str, start: usize, end: usize) -> bool {
    let mut depth = 0;
    let mut i = start + 1;

    while i < end - 1 {
        let ch = input[i..].chars().next().unwrap();
        if ch == '"' || ch == '\'' {
            if let Some(next) = find_matching_delimiter(input, i, ch, ch) {
                i = next;
                continue;
            }
            break;
        }

        if ch == '[' || ch == '{' || ch == '(' {
            depth += 1;
            i += ch.len_utf8();
            continue;
        }
        if ch == ']' || ch == '}' || ch == ')' {
            if depth > 0 {
                depth -= 1;
            }
            i += ch.len_utf8();
            continue;
        }

        if depth == 0 && ch == '|' {
            let next = skip_whitespace(input, i + 1);
            if next + 2 <= input.len() && &input[next..next + 2] == "in" {
                return true;
            }
        }

        i += ch.len_utf8();
    }

    false
}

fn has_invalid_literal_bracket_array(input: &str) -> Option<usize> {
    let mut i = 0;
    while i < input.len() {
        if input[i..].starts_with("//") {
            i += 2;
            while i < input.len() {
                let ch = input[i..].chars().next().unwrap();
                i += ch.len_utf8();
                if ch == '\n' {
                    break;
                }
            }
            continue;
        }

        if input[i..].starts_with("/*") {
            if let Some(end_comment) = input[i + 2..].find("*/") {
                i += 2 + end_comment + 2;
                continue;
            }
            break;
        }

        let ch = input[i..].chars().next().unwrap();
        if ch == '"' || ch == '\'' {
            if let Some(next) = find_matching_delimiter(input, i, ch, ch) {
                i = next;
                continue;
            }
            break;
        }

        if ch == '[' {
            let prev = prev_non_whitespace_char(input, i);
            if matches!(prev, Some(c) if c.is_ascii_alphanumeric() || c == ')' || c == ']' || c == '_') {
                i += ch.len_utf8();
                continue;
            }

            if let Some(end_bracket) = find_matching_delimiter(input, i, '[', ']') {
                if !is_array_comprehension(input, i, end_bracket) {
                    return Some(i);
                }
                i = end_bracket;
                continue;
            }
        }

        i += ch.len_utf8();
    }
    None
}

fn has_invalid_parenthesized_array_initializer(input: &str) -> Option<usize> {
    let mut i = 0;
    while i < input.len() {
        if input[i..].starts_with("//") {
            i += 2;
            while i < input.len() && input[i..].chars().next().unwrap() != '\n' {
                i += input[i..].chars().next().unwrap().len_utf8();
            }
            continue;
        }

        if input[i..].starts_with("/*") {
            if let Some(end_comment) = input[i + 2..].find("*/") {
                i += 2 + end_comment + 2;
                continue;
            } else {
                break;
            }
        }

        let ch = input[i..].chars().next().unwrap();
        if ch == '"' || ch == '\'' {
            if let Some(end_pos) = find_matching_delimiter(input, i, ch, ch) {
                i = end_pos;
                continue;
            } else {
                break;
            }
        }

        if input[i..].starts_with("new") {
            let after_new = i + 3;
            if after_new < input.len() {
                let next = input[after_new..].chars().next().unwrap();
                if next.is_ascii_alphanumeric() || next == '_' {
                    i += 3;
                    continue;
                }
            }

            i = skip_whitespace(input, after_new);
            // Skip base type and optional type suffixes until we reach an array suffix or non-type chars.
            while i < input.len() {
                let ch2 = input[i..].chars().next().unwrap();
                if ch2.is_whitespace()
                    || matches!(ch2, ':' | ',' | '(' | ')' | '{' | '}' | '[' | ']')
                {
                    break;
                }
                i += ch2.len_utf8();
            }

            loop {
                i = skip_whitespace(input, i);
                if input[i..].starts_with("[]") {
                    i += 2;
                    continue;
                }
                if i < input.len() && input[i..].starts_with('[') {
                    if let Some(end_bracket) = find_matching_delimiter(input, i, '[', ']') {
                        i = skip_whitespace(input, end_bracket);
                        if i < input.len() && input[i..].starts_with('(') {
                            return Some(i);
                        }
                        continue;
                    }
                }
                break;
            }
        }

        i += ch.len_utf8();
    }
    None
}

pub fn validate_array_initializer_syntax(input: &str) -> Result<(), String> {
    if let Some(pos) = has_invalid_parenthesized_array_initializer(input) {
        return Err(format!(
            "Sintaxis invalida: use '{{ ... }}' para inicializadores de arrays con lambda en lugar de '(...)' en la posicion {}",
            pos
        ));
    }
    if let Some(pos) = has_invalid_literal_bracket_array(input) {
        return Err(format!(
            "Sintaxis invalida: use '{{ ... }}' para literales de array en lugar de '[...]' en la posicion {}",
            pos
        ));
    }
    Ok(())
}

pub fn validate_before_lexer(input: &str) -> Result<String, (usize, String)> {
    validate_array_initializer_syntax(input).map_err(|message| (0, message))?;
    Ok(preprocess_new_array_initializers(input))
}

pub fn lex_safe(input: &str) -> Result<Vec<Token>, (usize, String)> {
    let preprocessed = validate_before_lexer(input)?;
    let tokens = tokenize_source(&preprocessed)?;
    Ok(postprocess_tokens(tokens))
}

fn tokenize_source(input: &str) -> Result<Vec<Token>, (usize, String)> {
    use crate::lexer::Token as LexerToken;
    use logos::Logos;

    let mut tokens = Vec::new();
    let mut lexer = LexerToken::lexer(input);

    while let Some(result) = lexer.next() {
        match result {
            Ok(tok) => tokens.push(tok),
            Err(_) => {
                return Err((
                    lexer.span().start,
                    format!("Token no reconocido: {:?}", lexer.slice()),
                ));
            }
        }
    }

    Ok(tokens)
}

pub fn postprocess_tokens(tokens: Vec<Token>) -> Vec<Token> {
    let tokens = remove_double_pipe_tokens(tokens);
    let tokens = wrap_inline_if_after_binary_ops(tokens);
    let tokens = fix_list_comprehension_pipe(tokens);
    tokens
}

pub fn preprocess_new_array_initializers(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    let len = chars.len();

    while i < len {
        // Pattern 1: `new Type[expr]{...}` -> convert outer braces to parens for lambda
        if i + 3 <= len && &input[i..i+3] == "new" {
            out.push_str("new");
            i += 3;

            // copy until we encounter a ']' that closes the size expression
            while i < len {
                let c = chars[i];
                out.push(c);
                i += 1;
                if c == ']' {
                    // copy whitespace
                    while i < len && chars[i].is_whitespace() {
                        out.push(chars[i]);
                        i += 1;
                    }
                    // if next char is '{', convert to '(' for lambda initializer
                    if i < len && chars[i] == '{' {
                        out.push('(');
                        i += 1; // consume the '{'
                        // copy until matching '}'
                        let mut nesting = 1usize;
                        while i < len && nesting > 0 {
                            let c2 = chars[i];
                            if c2 == '{' {
                                nesting += 1;
                                out.push(c2);
                            } else if c2 == '}' {
                                nesting -= 1;
                                if nesting == 0 {
                                    out.push(')');
                                } else {
                                    out.push('}');
                                }
                            } else {
                                out.push(c2);
                            }
                            i += 1;
                        }
                    }
                    break;
                }
            }
            continue;
        }

        // Pattern 2: Convert `{ ... }` to `[ ... ]` for array literals (comma-separated, no semicolons inside)
        if chars[i] == '{' {
            // Check if this brace contains commas (array) vs semicolons (block)
            let mut j = i + 1;
            let mut has_comma = false;
            let mut has_semicolon = false;
            let mut nesting = 1usize;

            while j < len && nesting > 0 {
                let cj = chars[j];
                if cj == '{' {
                    nesting += 1;
                } else if cj == '}' {
                    nesting -= 1;
                } else if nesting == 1 && cj == ',' {
                    has_comma = true;
                } else if nesting == 1 && cj == ';' {
                    has_semicolon = true;
                }
                j += 1;
            }

            // If it has commas and no semicolons, treat as array literal
            if has_comma && !has_semicolon {
                out.push('[');
                i += 1; // skip the '{'
                let mut depth = 1usize;
                while i < len && depth > 0 {
                    let c = chars[i];
                    if c == '{' {
                        depth += 1;
                        out.push('[');
                    } else if c == '}' {
                        depth -= 1;
                        if depth == 0 {
                            out.push(']');
                        } else {
                            out.push(']');
                        }
                    } else {
                        out.push(c);
                    }
                    i += 1;
                }
                continue;
            }
        }

        // default: copy char
        out.push(chars[i]);
        i += 1;
    }

    out
}
