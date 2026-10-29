use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

fn print_response(request: &str) -> bool {
    let answer = atlas_service::dispatch_json(request);
    println!("{answer}");
    serde_json::from_str::<Value>(&answer)
        .ok()
        .and_then(|v| v["ok"].as_bool())
        .unwrap_or(false)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "--version" | "-V") {
        println!("WhereIsIt AI {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.len() == 1 && args[0] == "--sqlite-version" {
        println!("{}", atlas_service::embedded_sqlite_version());
        return;
    }
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("WhereIsIt AI {}\n\natlas request '<JSON>'\natlas serve\natlas validate <catalog.sqlite>\natlas compact <source.sqlite> <new-copy.sqlite>\n\nCompact creates a verified v3 COPY and never replaces the source or an existing output.\nServe accepts one JSON request per line. No Python or database server is required.", env!("CARGO_PKG_VERSION"));
        return;
    }
    match args[0].as_str() {
        "serve" if args.len() == 1 => {
            let mut out = io::stdout().lock();
            for line in io::stdin().lock().lines() {
                match line {
                    Ok(line) if !line.trim().is_empty() => {
                        if writeln!(out, "{}", atlas_service::dispatch_json(&line))
                            .and_then(|_| out.flush())
                            .is_err()
                        {
                            std::process::exit(1);
                        }
                    }
                    Ok(_) => (),
                    Err(e) => {
                        eprintln!("stdin: {e}");
                        std::process::exit(1);
                    }
                }
            }
        }
        "request" if args.len() == 2 => {
            if !print_response(&args[1]) {
                std::process::exit(1);
            }
        }
        "validate" if args.len() == 2 => {
            let request = json!({"op":"validate_catalog","db":args[1]});
            if !print_response(&request.to_string()) {
                std::process::exit(1);
            }
        }
        "compact" if args.len() == 3 => {
            let request = json!({"op":"compact_catalog","db":args[1],"path":args[2]});
            if !print_response(&request.to_string()) {
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("Invalid command or arguments; use --help.");
            std::process::exit(2);
        }
    }
}
