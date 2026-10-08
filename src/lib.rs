//! mini-sql-engine: a small SQL query engine.
//!
//! Pipeline: SQL text -> lexer -> parser (AST) -> planner (logical plan)
//!           -> optimizer -> executor (Volcano iterators) -> rows

pub mod ast;
pub mod executor;
pub mod lexer;
pub mod optimizer;
pub mod parser;
pub mod plan;
pub mod planner;

pub use plan::Catalog;

/// Runs one SQL statement and returns the text to print.
pub fn run(sql: &str, catalog: &Catalog) -> Result<String, String> {
    let stmt = parser::parse(sql)?;
    let plan = planner::create_logical_plan(&stmt.select, catalog)?;
    let optimized = optimizer::optimize(plan.clone());

    if stmt.explain {
        return Ok(format!("Logical plan:\n{}\nOptimized plan:\n{}", plan, optimized));
    }

    let (columns, rows) = executor::execute(&optimized, catalog)?;
    Ok(format_table(&columns, &rows))
}

fn format_table(columns: &[String], rows: &[executor::Row]) -> String {
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| row.iter().map(|v| v.to_string()).collect())
        .collect();
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(i, name)| cells.iter().map(|r| r[i].len()).chain([name.len()]).max().unwrap_or(0))
        .collect();

    let line = |values: &[String]| -> String {
        values
            .iter()
            .zip(&widths)
            .map(|(v, w)| format!("{:<width$}", v, width = w))
            .collect::<Vec<_>>()
            .join(" | ")
            .trim_end()
            .to_string()
    };

    let mut out = String::new();
    out.push_str(&line(columns));
    out.push('\n');
    out.push_str(&widths.iter().map(|w| "-".repeat(*w)).collect::<Vec<_>>().join("-+-"));
    out.push('\n');
    for row in &cells {
        out.push_str(&line(row));
        out.push('\n');
    }
    let n = rows.len();
    out.push_str(&format!("({} row{})", n, if n == 1 { "" } else { "s" }));
    out
}
