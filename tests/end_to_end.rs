use mini_sql_engine::{run, Catalog};

fn catalog() -> Catalog {
    Catalog::new(concat!(env!("CARGO_MANIFEST_DIR"), "/data"))
}

fn query(sql: &str) -> String {
    run(sql, &catalog()).unwrap_or_else(|e| panic!("query failed: {}", e))
}

#[test]
fn select_star_returns_every_row() {
    assert!(query("SELECT * FROM employees").ends_with("(10 rows)"));
}

#[test]
fn filters_and_projects() {
    let out = query("SELECT name, salary FROM employees WHERE department = 'Engineering' AND salary > 120000");
    assert!(out.starts_with("name"));
    assert!(out.contains("Alice") && out.contains("Carol") && out.contains("Eve"));
    assert!(!out.contains("Heidi")); // Engineering but salary 112000
    assert!(!out.contains("Engineering")); // department column was not selected
    assert!(out.ends_with("(3 rows)"));
}

#[test]
fn or_and_parentheses() {
    let out = query("SELECT name FROM employees WHERE (department = 'Sales' OR department = 'Support') AND age < 30");
    assert!(out.contains("Frank") && out.contains("Ivan"));
    assert!(out.ends_with("(2 rows)"));
}

#[test]
fn limit_stops_early() {
    assert!(query("SELECT id FROM employees LIMIT 3").ends_with("(3 rows)"));
    assert!(query("SELECT id FROM employees LIMIT 0").ends_with("(0 rows)"));
}

#[test]
fn explain_shows_optimized_plan() {
    let out = query("EXPLAIN SELECT name FROM employees WHERE salary > 100000");
    assert!(out.contains("Filter: salary > 100000"));
    assert!(out.contains("Scan: employees columns=[name, salary] filter=salary > 100000"));
}

#[test]
fn reports_unknown_table_and_column() {
    assert!(run("SELECT * FROM nope", &catalog()).unwrap_err().contains("table 'nope' not found"));
    assert!(run("SELECT bonus FROM employees", &catalog()).unwrap_err().contains("column 'bonus'"));
}
