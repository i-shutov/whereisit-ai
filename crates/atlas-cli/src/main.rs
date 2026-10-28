use std::io::{self, BufRead};
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "--help" || a == "-h") {
        println!("WhereIsIt AI 0.1\n\natlas request '<JSON>'\natlas serve   # newline-delimited JSON on stdin/stdout\n\nExample: atlas request '{{\"op\":\"stats\",\"db\":\"catalog.atlas.sqlite\"}}'");
        return;
    }
    if args[0] == "serve" {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) if !line.trim().is_empty() => {
                    println!("{}", atlas_service::dispatch_json(&line))
                }
                Ok(_) => (),
                Err(e) => {
                    eprintln!("stdin: {e}");
                    std::process::exit(1);
                }
            }
        }
    } else if args[0] == "request" && args.len() == 2 {
        let answer = atlas_service::dispatch_json(&args[1]);
        println!("{answer}");
        let ok = serde_json::from_str::<serde_json::Value>(&answer)
            .ok()
            .and_then(|v| v["ok"].as_bool())
            .unwrap_or(false);
        if !ok {
            std::process::exit(1);
        }
    } else {
        eprintln!("Expected 'request <JSON>' or 'serve'; use --help.");
        std::process::exit(2);
    }
}
