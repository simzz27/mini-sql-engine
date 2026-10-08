# mini-sql-engine

A small SQL query engine written from scratch in Rust, with no external dependencies. It parses SQL, builds a logical query plan, optimizes it with rule-based rewrites, and executes it with a Volcano-style (iterator) engine over CSV files.

**Status:** in progress. See the roadmap below.

## Example

```
$ cargo run -- "EXPLAIN SELECT name FROM employees WHERE department = 'Engineering' AND age < 35"

Logical plan:
Projection: name
  Filter: (department = 'Engineering' AND age < 35)
    Scan: employees columns=[id, name, department, salary, age]

Optimized plan:
Projection: name
  Scan: employees columns=[name, department, age] filter=(department = 'Engineering' AND age < 35)
```

The optimizer pushed the filter into the scan, so rows are rejected while the file is being read. It also pruned the scan down to the three columns the query actually uses.

```
$ cargo run -- "SELECT name, department, salary FROM employees WHERE salary > 100000 LIMIT 3"

name  | department  | salary
------+-------------+-------
Alice | Engineering | 145000
Carol | Engineering | 128000
Eve   | Engineering | 165000
(3 rows)
```

## Usage

```
cargo run -- "<query>"            # run one query
cargo run                         # interactive shell (type 'exit' to quit)
cargo run -- --data <dir> "<query>"  # read tables from another folder
cargo test                        # run unit and end-to-end tests
```

Each table is a CSV file in `data/` (for example `data/employees.csv` is the table `employees`). The first line is the header. Fields that look like integers are treated as numbers; everything else is text.

## Supported SQL

- `SELECT *` or `SELECT col1, col2, ...`
- `FROM table`
- `WHERE` with `=`, `!=`/`<>`, `<`, `<=`, `>`, `>=`, `AND`, `OR`, and parentheses
- `LIMIT n`
- `EXPLAIN` to print the plan before and after optimization

## Architecture

```
SQL text
  -> lexer.rs      tokens
  -> parser.rs     AST (recursive descent; AND binds tighter than OR)
  -> planner.rs    logical plan: Limit -> Projection -> Filter -> Scan, with column validation
  -> optimizer.rs  rule-based rewrites
  -> executor.rs   Volcano-style physical operators -> rows
```

**Optimizer rules**

1. **Predicate pushdown:** a `Filter` directly above a `Scan` is merged into the scan, so non-matching rows are dropped during the read.
2. **Projection pruning:** the scan reads and parses only the columns needed by the operators above it (the projected columns plus any used in the filter).

**Execution model:** every operator implements `next()`, which returns one row at a time. Parents pull rows from their children, so data streams through the plan instead of being loaded into memory. `Limit` stops pulling as soon as it has enough rows.

## Roadmap

- [ ] `ORDER BY`
- [ ] Aggregations (`COUNT`, `SUM`, `AVG`) and `GROUP BY`
- [ ] Joins (nested loop, then hash join)
- [ ] Quoted CSV fields and more data types (floats, NULL)
- [ ] Benchmarks on large CSV files to measure the effect of each optimization
