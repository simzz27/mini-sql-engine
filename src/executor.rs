//! Volcano-style (iterator) executor.
//!
//! Every physical operator implements `next()`, which returns one row at a
//! time. A parent pulls rows from its child, so rows stream through the plan
//! without loading the whole table into memory.

use crate::ast::{BinaryOp, Expr, Value};
use crate::plan::{Catalog, LogicalPlan};
use std::cmp::Ordering;
use std::fs::File;
use std::io::{BufRead, BufReader, Lines};

pub type Row = Vec<Value>;

pub trait Operator {
    /// Names of the columns in each row this operator returns.
    fn schema(&self) -> &[String];
    /// The next row, or `None` when there are no more rows.
    fn next(&mut self) -> Result<Option<Row>, String>;
}

/// Reads a CSV file line by line, keeping only the needed columns and
/// (if the optimizer pushed one down) applying a filter while scanning.
struct ScanExec {
    schema: Vec<String>,
    lines: Lines<BufReader<File>>,
    indices: Vec<usize>,
    filter: Option<Expr>,
    line_no: usize,
}

impl ScanExec {
    fn new(catalog: &Catalog, table: &str, columns: &[String], filter: Option<Expr>) -> Result<Self, String> {
        let path = catalog.table_path(table);
        let file = File::open(&path).map_err(|e| format!("cannot open {}: {}", path.display(), e))?;
        let mut lines = BufReader::new(file).lines();
        let header: Vec<String> = match lines.next() {
            Some(line) => line
                .map_err(|e| e.to_string())?
                .split(',')
                .map(|c| c.trim().to_string())
                .collect(),
            None => return Err(format!("table '{}' is empty", table)),
        };
        let indices = columns
            .iter()
            .map(|col| {
                header
                    .iter()
                    .position(|h| h == col)
                    .ok_or_else(|| format!("column '{}' not found in table '{}'", col, table))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ScanExec { schema: columns.to_vec(), lines, indices, filter, line_no: 1 })
    }
}

impl Operator for ScanExec {
    fn schema(&self) -> &[String] {
        &self.schema
    }

    fn next(&mut self) -> Result<Option<Row>, String> {
        loop {
            let line = match self.lines.next() {
                None => return Ok(None),
                Some(line) => line.map_err(|e| e.to_string())?,
            };
            self.line_no += 1;
            if line.trim().is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split(',').collect();
            let mut row = Vec::with_capacity(self.indices.len());
            for &i in &self.indices {
                let field = fields
                    .get(i)
                    .ok_or_else(|| format!("line {} has only {} fields", self.line_no, fields.len()))?;
                // Only the needed columns are parsed (projection pruning pays off here).
                row.push(Value::parse(field));
            }
            if let Some(filter) = &self.filter {
                if !eval_predicate(filter, &row, &self.schema)? {
                    continue;
                }
            }
            return Ok(Some(row));
        }
    }
}

struct FilterExec {
    input: Box<dyn Operator>,
    predicate: Expr,
}

impl Operator for FilterExec {
    fn schema(&self) -> &[String] {
        self.input.schema()
    }

    fn next(&mut self) -> Result<Option<Row>, String> {
        while let Some(row) = self.input.next()? {
            if eval_predicate(&self.predicate, &row, self.input.schema())? {
                return Ok(Some(row));
            }
        }
        Ok(None)
    }
}

struct ProjectionExec {
    input: Box<dyn Operator>,
    schema: Vec<String>,
    indices: Vec<usize>,
}

impl ProjectionExec {
    fn new(input: Box<dyn Operator>, columns: &[String]) -> Result<Self, String> {
        let indices = columns
            .iter()
            .map(|col| column_index(col, input.schema()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ProjectionExec { input, schema: columns.to_vec(), indices })
    }
}

impl Operator for ProjectionExec {
    fn schema(&self) -> &[String] {
        &self.schema
    }

    fn next(&mut self) -> Result<Option<Row>, String> {
        Ok(self
            .input
            .next()?
            .map(|row| self.indices.iter().map(|&i| row[i].clone()).collect()))
    }
}

struct LimitExec {
    input: Box<dyn Operator>,
    remaining: usize,
}

impl Operator for LimitExec {
    fn schema(&self) -> &[String] {
        self.input.schema()
    }

    fn next(&mut self) -> Result<Option<Row>, String> {
        // Stop pulling from the child as soon as the limit is reached.
        if self.remaining == 0 {
            return Ok(None);
        }
        let row = self.input.next()?;
        if row.is_some() {
            self.remaining -= 1;
        }
        Ok(row)
    }
}

fn column_index(col: &str, schema: &[String]) -> Result<usize, String> {
    schema
        .iter()
        .position(|c| c == col)
        .ok_or_else(|| format!("column '{}' is not available here", col))
}

fn eval_value(expr: &Expr, row: &Row, schema: &[String]) -> Result<Value, String> {
    match expr {
        Expr::Column(name) => Ok(row[column_index(name, schema)?].clone()),
        Expr::Literal(v) => Ok(v.clone()),
        Expr::Binary { .. } => Err(format!("expected a value, found condition '{}'", expr)),
    }
}

pub fn eval_predicate(expr: &Expr, row: &Row, schema: &[String]) -> Result<bool, String> {
    match expr {
        Expr::Binary { left, op: BinaryOp::And, right } => {
            Ok(eval_predicate(left, row, schema)? && eval_predicate(right, row, schema)?)
        }
        Expr::Binary { left, op: BinaryOp::Or, right } => {
            Ok(eval_predicate(left, row, schema)? || eval_predicate(right, row, schema)?)
        }
        Expr::Binary { left, op, right } => {
            let l = eval_value(left, row, schema)?;
            let r = eval_value(right, row, schema)?;
            // Values of different types are never equal or ordered.
            let ordering = match (&l, &r) {
                (Value::Int(a), Value::Int(b)) => a.cmp(b),
                (Value::Text(a), Value::Text(b)) => a.cmp(b),
                _ => return Ok(*op == BinaryOp::NotEq),
            };
            Ok(match op {
                BinaryOp::Eq => ordering == Ordering::Equal,
                BinaryOp::NotEq => ordering != Ordering::Equal,
                BinaryOp::Lt => ordering == Ordering::Less,
                BinaryOp::LtEq => ordering != Ordering::Greater,
                BinaryOp::Gt => ordering == Ordering::Greater,
                BinaryOp::GtEq => ordering != Ordering::Less,
                BinaryOp::And | BinaryOp::Or => unreachable!(),
            })
        }
        _ => Err(format!("WHERE needs a condition, found '{}'", expr)),
    }
}

/// Turns a logical plan into a tree of physical operators.
pub fn build(plan: &LogicalPlan, catalog: &Catalog) -> Result<Box<dyn Operator>, String> {
    Ok(match plan {
        LogicalPlan::Scan { table, columns, filter } => {
            Box::new(ScanExec::new(catalog, table, columns, filter.clone())?)
        }
        LogicalPlan::Filter { predicate, input } => Box::new(FilterExec {
            input: build(input, catalog)?,
            predicate: predicate.clone(),
        }),
        LogicalPlan::Projection { columns, input } => {
            Box::new(ProjectionExec::new(build(input, catalog)?, columns)?)
        }
        LogicalPlan::Limit { n, input } => Box::new(LimitExec { input: build(input, catalog)?, remaining: *n }),
    })
}

/// Runs a plan to completion and returns (column names, rows).
pub fn execute(plan: &LogicalPlan, catalog: &Catalog) -> Result<(Vec<String>, Vec<Row>), String> {
    let mut root = build(plan, catalog)?;
    let mut rows = Vec::new();
    while let Some(row) = root.next()? {
        rows.push(row);
    }
    Ok((root.schema().to_vec(), rows))
}
