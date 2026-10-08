//! Catalog (where tables live) and the logical plan tree.

use crate::ast::Expr;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

/// Each table is a CSV file `<data_dir>/<table>.csv` whose first line is the header.
pub struct Catalog {
    data_dir: PathBuf,
}

impl Catalog {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Catalog { data_dir: data_dir.into() }
    }

    pub fn table_path(&self, table: &str) -> PathBuf {
        self.data_dir.join(format!("{}.csv", table))
    }

    /// Reads only the header line to get the table's column names.
    pub fn schema(&self, table: &str) -> Result<Vec<String>, String> {
        let path = self.table_path(table);
        let file = File::open(&path)
            .map_err(|_| format!("table '{}' not found (looked for {})", table, path.display()))?;
        let mut header = String::new();
        BufReader::new(file)
            .read_line(&mut header)
            .map_err(|e| e.to_string())?;
        let columns: Vec<String> = header
            .trim()
            .split(',')
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty())
            .collect();
        if columns.is_empty() {
            return Err(format!("table '{}' has no header row", table));
        }
        Ok(columns)
    }
}

/// A tree of relational operators describing *what* to compute.
#[derive(Debug, Clone, PartialEq)]
pub enum LogicalPlan {
    /// Read rows from a table. After optimization it may read only some
    /// columns and apply a filter while scanning.
    Scan {
        table: String,
        columns: Vec<String>,
        filter: Option<Expr>,
    },
    Filter {
        predicate: Expr,
        input: Box<LogicalPlan>,
    },
    Projection {
        columns: Vec<String>,
        input: Box<LogicalPlan>,
    },
    Limit {
        n: usize,
        input: Box<LogicalPlan>,
    },
}

impl LogicalPlan {
    fn fmt_indented(&self, f: &mut fmt::Formatter, depth: usize) -> fmt::Result {
        let pad = "  ".repeat(depth);
        match self {
            LogicalPlan::Scan { table, columns, filter } => {
                write!(f, "{}Scan: {} columns=[{}]", pad, table, columns.join(", "))?;
                if let Some(pred) = filter {
                    write!(f, " filter={}", pred)?;
                }
                writeln!(f)
            }
            LogicalPlan::Filter { predicate, input } => {
                writeln!(f, "{}Filter: {}", pad, predicate)?;
                input.fmt_indented(f, depth + 1)
            }
            LogicalPlan::Projection { columns, input } => {
                writeln!(f, "{}Projection: {}", pad, columns.join(", "))?;
                input.fmt_indented(f, depth + 1)
            }
            LogicalPlan::Limit { n, input } => {
                writeln!(f, "{}Limit: {}", pad, n)?;
                input.fmt_indented(f, depth + 1)
            }
        }
    }
}

impl fmt::Display for LogicalPlan {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_indented(f, 0)
    }
}
