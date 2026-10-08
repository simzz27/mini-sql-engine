//! Rule-based optimizer. Each rule rewrites the logical plan tree.
//!
//! 1. Predicate pushdown: move a Filter that sits on a Scan *into* the Scan,
//!    so rows are rejected while reading instead of in a later operator.
//! 2. Projection pruning: make the Scan read only the columns that some
//!    operator above it actually needs.

use crate::ast::{BinaryOp, Expr};
use crate::plan::LogicalPlan;

pub fn optimize(plan: LogicalPlan) -> LogicalPlan {
    let plan = push_down_predicates(plan);
    prune_columns(plan, None)
}

fn push_down_predicates(plan: LogicalPlan) -> LogicalPlan {
    match plan {
        LogicalPlan::Filter { predicate, input } => match push_down_predicates(*input) {
            LogicalPlan::Scan { table, columns, filter } => {
                // Combine with any filter the scan already has.
                let filter = match filter {
                    Some(existing) => Expr::Binary {
                        left: Box::new(existing),
                        op: BinaryOp::And,
                        right: Box::new(predicate),
                    },
                    None => predicate,
                };
                LogicalPlan::Scan { table, columns, filter: Some(filter) }
            }
            other => LogicalPlan::Filter { predicate, input: Box::new(other) },
        },
        LogicalPlan::Projection { columns, input } => LogicalPlan::Projection {
            columns,
            input: Box::new(push_down_predicates(*input)),
        },
        LogicalPlan::Limit { n, input } => LogicalPlan::Limit {
            n,
            input: Box::new(push_down_predicates(*input)),
        },
        scan @ LogicalPlan::Scan { .. } => scan,
    }
}

/// `required` is the set of columns the parent operator needs (`None` = all).
fn prune_columns(plan: LogicalPlan, required: Option<Vec<String>>) -> LogicalPlan {
    match plan {
        LogicalPlan::Limit { n, input } => LogicalPlan::Limit {
            n,
            input: Box::new(prune_columns(*input, required)),
        },
        LogicalPlan::Projection { columns, input } => {
            // Below a projection, only the projected columns are needed.
            let input = prune_columns(*input, Some(columns.clone()));
            LogicalPlan::Projection { columns, input: Box::new(input) }
        }
        LogicalPlan::Filter { predicate, input } => {
            let required = required.map(|mut cols| {
                predicate.columns(&mut cols);
                cols
            });
            LogicalPlan::Filter { predicate, input: Box::new(prune_columns(*input, required)) }
        }
        LogicalPlan::Scan { table, columns, filter } => {
            let columns = match required {
                None => columns,
                Some(mut needed) => {
                    if let Some(f) = &filter {
                        f.columns(&mut needed);
                    }
                    // Keep the table's original column order.
                    columns.into_iter().filter(|c| needed.contains(c)).collect()
                }
            };
            LogicalPlan::Scan { table, columns, filter }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Value;

    fn scan() -> LogicalPlan {
        LogicalPlan::Scan {
            table: "employees".into(),
            columns: vec!["id".into(), "name".into(), "department".into(), "salary".into()],
            filter: None,
        }
    }

    fn salary_over(n: i64) -> Expr {
        Expr::Binary {
            left: Box::new(Expr::Column("salary".into())),
            op: BinaryOp::Gt,
            right: Box::new(Expr::Literal(Value::Int(n))),
        }
    }

    #[test]
    fn pushes_filter_into_scan_and_prunes_columns() {
        let plan = LogicalPlan::Projection {
            columns: vec!["name".into()],
            input: Box::new(LogicalPlan::Filter { predicate: salary_over(100), input: Box::new(scan()) }),
        };
        let expected = LogicalPlan::Projection {
            columns: vec!["name".into()],
            input: Box::new(LogicalPlan::Scan {
                table: "employees".into(),
                columns: vec!["name".into(), "salary".into()],
                filter: Some(salary_over(100)),
            }),
        };
        assert_eq!(optimize(plan), expected);
    }

    #[test]
    fn select_star_keeps_all_columns() {
        let plan = LogicalPlan::Projection {
            columns: vec!["id".into(), "name".into(), "department".into(), "salary".into()],
            input: Box::new(scan()),
        };
        assert_eq!(optimize(plan.clone()), plan);
    }
}
