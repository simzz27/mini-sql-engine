//! Planner: turns a parsed SELECT statement into an (unoptimized) logical plan.
//!
//! The naive plan always has the same shape:
//!   Limit -> Projection -> Filter -> Scan (all columns)

use crate::ast::{SelectItems, SelectStatement};
use crate::plan::{Catalog, LogicalPlan};

pub fn create_logical_plan(stmt: &SelectStatement, catalog: &Catalog) -> Result<LogicalPlan, String> {
    let schema = catalog.schema(&stmt.table)?;
    let check_column = |col: &str| -> Result<(), String> {
        if schema.iter().any(|c| c == col) {
            Ok(())
        } else {
            Err(format!("column '{}' not found in table '{}'", col, stmt.table))
        }
    };

    let mut plan = LogicalPlan::Scan {
        table: stmt.table.clone(),
        columns: schema.clone(),
        filter: None,
    };

    if let Some(predicate) = &stmt.selection {
        let mut referenced = Vec::new();
        predicate.columns(&mut referenced);
        for col in &referenced {
            check_column(col)?;
        }
        plan = LogicalPlan::Filter { predicate: predicate.clone(), input: Box::new(plan) };
    }

    let columns = match &stmt.items {
        SelectItems::Star => schema.clone(),
        SelectItems::Columns(cols) => {
            for col in cols {
                check_column(col)?;
            }
            cols.clone()
        }
    };
    plan = LogicalPlan::Projection { columns, input: Box::new(plan) };

    if let Some(n) = stmt.limit {
        plan = LogicalPlan::Limit { n, input: Box::new(plan) };
    }

    Ok(plan)
}
