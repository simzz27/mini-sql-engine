//! Recursive-descent parser: turns tokens into a `Statement`.
//!
//! Grammar (simplified):
//!   statement  := [EXPLAIN] SELECT items FROM ident [WHERE expr] [LIMIT number] [;]
//!   items      := '*' | ident (',' ident)*
//!   expr       := and_expr (OR and_expr)*
//!   and_expr   := comparison (AND comparison)*
//!   comparison := primary [(= | != | < | <= | > | >=) primary]
//!   primary    := ident | number | string | '(' expr ')'

use crate::ast::{BinaryOp, Expr, SelectItems, SelectStatement, Statement, Value};
use crate::lexer::{tokenize, Token};

pub fn parse(sql: &str) -> Result<Statement, String> {
    let tokens = tokenize(sql)?;
    let mut parser = Parser { tokens, pos: 0 };
    parser.parse_statement()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    /// Consumes the next token if it equals `token`.
    fn consume_if(&mut self, token: &Token) -> bool {
        if self.peek() == Some(token) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), String> {
        match self.advance() {
            Some(t) if t == expected => Ok(()),
            Some(t) => Err(format!("expected {:?}, found {:?}", expected, t)),
            None => Err(format!("expected {:?}, found end of query", expected)),
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, String> {
        let explain = self.consume_if(&Token::Explain);
        self.expect(Token::Select)?;
        let items = self.parse_select_items()?;
        self.expect(Token::From)?;
        let table = self.parse_ident()?;

        let selection = if self.consume_if(&Token::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let limit = if self.consume_if(&Token::Limit) {
            match self.advance() {
                Some(Token::Number(n)) if n >= 0 => Some(n as usize),
                other => return Err(format!("expected a non-negative number after LIMIT, found {:?}", other)),
            }
        } else {
            None
        };

        self.consume_if(&Token::Semicolon);
        if let Some(token) = self.peek() {
            return Err(format!("unexpected {:?} after end of query", token));
        }

        Ok(Statement {
            explain,
            select: SelectStatement { items, table, selection, limit },
        })
    }

    fn parse_select_items(&mut self) -> Result<SelectItems, String> {
        if self.consume_if(&Token::Star) {
            return Ok(SelectItems::Star);
        }
        let mut columns = vec![self.parse_ident()?];
        while self.consume_if(&Token::Comma) {
            columns.push(self.parse_ident()?);
        }
        Ok(SelectItems::Columns(columns))
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        match self.advance() {
            Some(Token::Ident(name)) => Ok(name),
            Some(t) => Err(format!("expected a name, found {:?}", t)),
            None => Err("expected a name, found end of query".to_string()),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.consume_if(&Token::Or) {
            let right = self.parse_and()?;
            left = Expr::Binary { left: Box::new(left), op: BinaryOp::Or, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_comparison()?;
        while self.consume_if(&Token::And) {
            let right = self.parse_comparison()?;
            left = Expr::Binary { left: Box::new(left), op: BinaryOp::And, right: Box::new(right) };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let left = self.parse_primary()?;
        let op = match self.peek() {
            Some(Token::Eq) => BinaryOp::Eq,
            Some(Token::NotEq) => BinaryOp::NotEq,
            Some(Token::Lt) => BinaryOp::Lt,
            Some(Token::LtEq) => BinaryOp::LtEq,
            Some(Token::Gt) => BinaryOp::Gt,
            Some(Token::GtEq) => BinaryOp::GtEq,
            _ => return Ok(left),
        };
        self.pos += 1;
        let right = self.parse_primary()?;
        Ok(Expr::Binary { left: Box::new(left), op, right: Box::new(right) })
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match self.advance() {
            Some(Token::Ident(name)) => Ok(Expr::Column(name)),
            Some(Token::Number(n)) => Ok(Expr::Literal(Value::Int(n))),
            Some(Token::StringLit(s)) => Ok(Expr::Literal(Value::Text(s))),
            Some(Token::LParen) => {
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(expr)
            }
            Some(t) => Err(format!("unexpected {:?} in expression", t)),
            None => Err("unexpected end of query in expression".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_select() {
        let stmt = parse("SELECT name, salary FROM employees WHERE salary > 100000 LIMIT 3;").unwrap();
        assert!(!stmt.explain);
        assert_eq!(stmt.select.table, "employees");
        assert_eq!(stmt.select.items, SelectItems::Columns(vec!["name".into(), "salary".into()]));
        assert_eq!(stmt.select.limit, Some(3));
        assert_eq!(stmt.select.selection.unwrap().to_string(), "salary > 100000");
    }

    #[test]
    fn and_binds_tighter_than_or() {
        let stmt = parse("SELECT * FROM t WHERE a = 1 OR b = 2 AND c = 3").unwrap();
        assert_eq!(stmt.select.selection.unwrap().to_string(), "(a = 1 OR (b = 2 AND c = 3))");
    }

    #[test]
    fn parses_explain_and_parentheses() {
        let stmt = parse("EXPLAIN SELECT * FROM t WHERE (a = 1 OR b = 2) AND c = 3").unwrap();
        assert!(stmt.explain);
        assert_eq!(stmt.select.selection.unwrap().to_string(), "((a = 1 OR b = 2) AND c = 3)");
    }

    #[test]
    fn reports_errors() {
        assert!(parse("SELECT FROM t").is_err());
        assert!(parse("SELECT a FROM").is_err());
        assert!(parse("SELECT a FROM t WHERE").is_err());
        assert!(parse("SELECT a FROM t extra").is_err());
    }
}
