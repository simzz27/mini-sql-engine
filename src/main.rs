use mini_sql_engine::{run, Catalog};
use std::io::{self, BufRead, Write};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();

    // Optional: --data <dir> (defaults to ./data)
    let mut data_dir = "data".to_string();
    if let Some(pos) = args.iter().position(|a| a == "--data") {
        if pos + 1 >= args.len() {
            eprintln!("error: --data needs a directory");
            std::process::exit(1);
        }
        data_dir = args.remove(pos + 1);
        args.remove(pos);
    }
    let catalog = Catalog::new(data_dir);

    // One query passed as an argument: run it and exit.
    if !args.is_empty() {
        match run(&args.join(" "), &catalog) {
            Ok(output) => println!("{}", output),
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Otherwise start an interactive shell.
    println!("mini-sql-engine. Type a query, or 'exit' to quit.");
    let stdin = io::stdin();
    loop {
        print!("sql> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let query = line.trim();
        if query.is_empty() {
            continue;
        }
        if query.eq_ignore_ascii_case("exit") || query.eq_ignore_ascii_case("quit") {
            break;
        }
        match run(query, &catalog) {
            Ok(output) => println!("{}\n", output),
            Err(e) => println!("error: {}\n", e),
        }
    }
}
