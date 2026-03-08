use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use autoresearch_rs::{read_kv, AppResult};

fn parse_args() -> AppResult<PathBuf> {
    let mut runs_dir = PathBuf::from("runs");
    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--runs-dir" => runs_dir = PathBuf::from(it.next().ok_or("missing value")?),
            "--help" | "-h" => {
                println!("report usage: cargo run --bin report -- [--runs-dir runs]");
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    Ok(runs_dir)
}

fn print_block(name: &str, map: &HashMap<String, String>) {
    println!("{name}");
    for k in ["run_id", "val_bpb", "status", "checkpoint"] {
        if let Some(v) = map.get(k) {
            println!("  {k}: {v}");
        }
    }
}

fn main() -> AppResult<()> {
    let runs_dir = parse_args()?;

    let latest_path = runs_dir.join("latest.txt");
    let best_path = runs_dir.join("best.txt");

    if latest_path.exists() {
        let latest = read_kv(&latest_path)?;
        print_block("latest", &latest);
    } else {
        println!("latest: not found ({})", latest_path.display());
    }

    if best_path.exists() {
        let best = read_kv(&best_path)?;
        print_block("best", &best);
    } else {
        println!("best: not found ({})", best_path.display());
    }

    Ok(())
}
