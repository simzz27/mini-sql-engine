//! Lexer: turns a SQL string into a list of tokens.

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Select,
    From,
    Where,
    And,
    Or,
    Limit,
    Explain,
    // Values
    Ident(String),
    Number(i64),
    StringLit(String),
    // Punctuation
    Comma,
    Star,
    LParen,
    RParen,
    Semicolon,
    // Comparison operators
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();

        match c {
            c if c.is_whitespace() => i += 1,
            ',' => { tokens.push(Token::Comma); i += 1; }
            '*' => { tokens.push(Token::Star); i += 1; }
            '(' => { tokens.push(Token::LParen); i += 1; }
            ')' => { tokens.push(Token::RParen); i += 1; }
            ';' => { tokens.push(Token::Semicolon); i += 1; }
            '=' => { tokens.push(Token::Eq); i += 1; }
            '!' if next == Some('=') => { tokens.push(Token::NotEq); i += 2; }
            '<' if next == Some('=') => { tokens.push(Token::LtEq); i += 2; }
            '<' if next == Some('>') => { tokens.push(Token::NotEq); i += 2; }
            '<' => { tokens.push(Token::Lt); i += 1; }
            '>' if next == Some('=') => { tokens.push(Token::GtEq); i += 2; }
            '>' => { tokens.push(Token::Gt); i += 1; }
            '\'' => {
                // String literal: read until the closing quote.
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && chars[end] != '\'' {
                    end += 1;
                }
                if end == chars.len() {
                    return Err(format!("unterminated string starting at position {}", i));
                }
                tokens.push(Token::StringLit(chars[start..end].iter().collect()));
                i = end + 1;
            }
            c if c.is_ascii_digit() || (c == '-' && next.map_or(false, |n| n.is_ascii_digit())) => {
                let start = i;
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                let n = text
                    .parse::<i64>()
                    .map_err(|_| format!("invalid number '{}'", text))?;
                tokens.push(Token::Number(n));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                // Keywords are case-insensitive; identifiers keep their case.
                let token = match word.to_uppercase().as_str() {
                    "SELECT" => Token::Select,
                    "FROM" => Token::From,
                    "WHERE" => Token::Where,
                    "AND" => Token::And,
                    "OR" => Token::Or,
                    "LIMIT" => Token::Limit,
                    "EXPLAIN" => Token::Explain,
                    _ => Token::Ident(word),
                };
                tokens.push(token);
            }
            other => return Err(format!("unexpected character '{}' at position {}", other, i)),
        }
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_simple_query() {
        let tokens = tokenize("select name FROM employees where salary >= 100000").unwrap();
        assert_eq!(
            tokens,
            vec![
                Token::Select,
                Token::Ident("name".into()),
                Token::From,
                Token::Ident("employees".into()),
                Token::Where,
                Token::Ident("salary".into()),
                Token::GtEq,
                Token::Number(100000),
            ]
        );
    }

    #[test]
    fn tokenizes_strings_and_operators() {
        let tokens = tokenize("dept = 'Sales' AND age <> 30").unwrap();
        assert_eq!(tokens[2], Token::StringLit("Sales".into()));
        assert_eq!(tokens[5], Token::NotEq);
    }

    #[test]
    fn rejects_unterminated_string() {
        assert!(tokenize("name = 'Bob").is_err());
    }
}
