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

// ==========================================
// SOURCE TEXT PREPROCESSING
// ==========================================

/// Replace `{ ... }` with `( ... )` for `new Type[expr]{...}` (initializer with lambda).
/// Also replace `{ ... }` with `[ ... ]` for array literals like `{1,2,3}` (contains only commas, not semicolons).
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
