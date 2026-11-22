//! Tiny CLI for testing the engine:
//!   cargo run --release --example zipnest_cli -- zip <out.zip> <inputs...>
//!   cargo run --release --example zipnest_cli -- unzip <archive.zip> <dest-dir>
use std::path::PathBuf;
use zipnest_core::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut last = 0u64;
    let mut cb = |p: &Progress| {
        let pct = (p.processed_bytes * 100)
            .checked_div(p.total_bytes)
            .unwrap_or(0);
        if pct >= last + 10 {
            last = pct;
            eprintln!("{:?} {pct}%", p.phase);
        }
    };
    let res = match args.first().map(String::as_str) {
        Some("zip") if args.len() >= 3 => {
            let inputs: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
            compress(
                &inputs,
                args[1].as_ref(),
                &CompressOptions::default(),
                &CancelToken::new(),
                &mut cb,
            )
            .map(|s| format!("{s:#?}"))
        }
        Some("unzip") if args.len() == 3 => extract(
            args[1].as_ref(),
            args[2].as_ref(),
            &ExtractOptions::default(),
            &CancelToken::new(),
            &mut cb,
        )
        .map(|s| format!("{s:#?}")),
        _ => {
            eprintln!("usage: zipnest_cli zip <out.zip> <inputs...> | unzip <archive.zip> <dest>");
            std::process::exit(2)
        }
    };
    match res {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1)
        }
    }
}
