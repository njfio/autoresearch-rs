use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use autoresearch_rs::{read_kv, AppResult};

#[derive(Debug, Clone)]
struct Args {
    experiments: usize,
    artifacts_dir: PathBuf,
    runs_dir: PathBuf,
    time_budget_seconds: u64,
    base_batch_size: usize,
    base_seq_len: usize,
    base_learning_rate: f32,
    seed: u64,
}

fn parse_args() -> AppResult<Args> {
    let mut args = Args {
        experiments: 10,
        artifacts_dir: PathBuf::from("artifacts"),
        runs_dir: PathBuf::from("runs"),
        time_budget_seconds: 300,
        base_batch_size: 32,
        base_seq_len: 64,
        base_learning_rate: 1.0,
        seed: 1337,
    };

    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--experiments" => args.experiments = it.next().ok_or("missing value")?.parse()?,
            "--artifacts-dir" => {
                args.artifacts_dir = PathBuf::from(it.next().ok_or("missing value")?)
            }
            "--runs-dir" => args.runs_dir = PathBuf::from(it.next().ok_or("missing value")?),
            "--time-budget-seconds" => {
                args.time_budget_seconds = it.next().ok_or("missing value")?.parse()?
            }
            "--base-batch-size" => {
                args.base_batch_size = it.next().ok_or("missing value")?.parse()?
            }
            "--base-seq-len" => args.base_seq_len = it.next().ok_or("missing value")?.parse()?,
            "--base-learning-rate" => {
                args.base_learning_rate = it.next().ok_or("missing value")?.parse()?
            }
            "--seed" => args.seed = it.next().ok_or("missing value")?.parse()?,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    Ok(args)
}

fn print_help() {
    println!("autoresearch usage:");
    println!("  cargo run --bin autoresearch -- [options]");
    println!("options:");
    println!("  --experiments <n>            default: 10");
    println!("  --artifacts-dir <path>       default: artifacts");
    println!("  --runs-dir <path>            default: runs");
    println!("  --time-budget-seconds <s>    default: 300");
    println!("  --base-batch-size <n>        default: 32");
    println!("  --base-seq-len <n>           default: 64");
    println!("  --base-learning-rate <f>     default: 1.0");
    println!("  --seed <u64>                 default: 1337");
}

fn main() -> AppResult<()> {
    let args = parse_args()?;

    for exp_idx in 0..args.experiments {
        let s = args.seed.wrapping_add(exp_idx as u64 * 17);
        let batch_size = mutate_usize(args.base_batch_size, s, &[0.5, 1.0, 2.0]);
        let seq_len = mutate_usize(args.base_seq_len, s.rotate_left(13), &[0.5, 1.0, 2.0]);
        let learning_rate =
            mutate_f32(args.base_learning_rate, s.rotate_left(29), &[0.5, 1.0, 1.5]);

        let description = format!(
            "auto exp={} batch_size={} seq_len={} lr={:.4} seed={}",
            exp_idx + 1,
            batch_size,
            seq_len,
            learning_rate,
            s
        );

        println!("=== experiment {}/{} ===", exp_idx + 1, args.experiments);
        println!("{description}");

        let status = Command::new("cargo")
            .arg("run")
            .arg("--release")
            .arg("--bin")
            .arg("train")
            .arg("--")
            .arg("--artifacts-dir")
            .arg(&args.artifacts_dir)
            .arg("--runs-dir")
            .arg(&args.runs_dir)
            .arg("--time-budget-seconds")
            .arg(args.time_budget_seconds.to_string())
            .arg("--batch-size")
            .arg(batch_size.to_string())
            .arg("--seq-len")
            .arg(seq_len.to_string())
            .arg("--learning-rate")
            .arg(format!("{learning_rate:.6}"))
            .arg("--seed")
            .arg(s.to_string())
            .arg("--description")
            .arg(&description)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?;

        if !status.success() {
            return Err(format!("train run failed on experiment {}", exp_idx + 1).into());
        }

        let latest = read_kv(&args.runs_dir.join("latest.txt"))?;
        let run_id = latest
            .get("run_id")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let val_bpb = latest
            .get("val_bpb")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        let keep = latest
            .get("status")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());
        println!("result: run_id={run_id} val_bpb={val_bpb} status={keep}");
    }

    println!(
        "autoresearch loop finished: {} experiments",
        args.experiments
    );
    Ok(())
}

fn mutate_usize(base: usize, state: u64, factors: &[f32]) -> usize {
    let idx = (state as usize) % factors.len();
    let v = (base as f32 * factors[idx]).round().max(1.0) as usize;
    v.max(1)
}

fn mutate_f32(base: f32, state: u64, factors: &[f32]) -> f32 {
    let idx = (state as usize) % factors.len();
    (base * factors[idx]).max(1e-6)
}
