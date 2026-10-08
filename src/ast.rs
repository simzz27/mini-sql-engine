//! Abstract syntax tree (AST) produced by the parser.

use std::fmt;

/// A single value in a row. CSV fields that parse as integers become `Int`,
/// everything else becomes `Text`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Text(String),
}

impl Value {
    pub fn parse(raw: &str) -> Value {
        let raw = raw.trim();
        match raw.parse::<i64>() {
            Ok(n) => Value::Int(n),
            Err(_) => Value::Text(raw.to_string()),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Int(n) => write!(f, "{}", n),
            Value::Text(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match self {
            BinaryOp::Eq => "=",
            BinaryOp::NotEq => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::LtEq => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::GtEq => ">=",
            BinaryOp::And => "AND",
            BinaryOp::Or => "OR",
        };
        write!(f, "{}", s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Column(String),
    Literal(Value),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
    },
}

impl Expr {
    /// Collects every column name this expression references (no duplicates).
    pub fn columns(&self, out: &mut Vec<String>) {
        match self {
            Expr::Column(name) => {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
            Expr::Literal(_) => {}
            Expr::Binary { left, right, .. } => {
                left.columns(out);
                right.columns(out);
            }
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expr::Column(name) => write!(f, "{}", name),
            Expr::Literal(Value::Int(n)) => write!(f, "{}", n),
            Expr::Literal(Value::Text(s)) => write!(f, "'{}'", s),
            Expr::Binary { left, op: op @ (BinaryOp::And | BinaryOp::Or), right } => {
                write!(f, "({} {} {})", left, op, right)
            }
            Expr::Binary { left, op, right } => write!(f, "{} {} {}", left, op, right),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItems {
    Star,
    Columns(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectStatement {
    pub items: SelectItems,
    pub table: String,
    pub selection: Option<Expr>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub explain: bool,
    pub select: SelectStatement,
}
