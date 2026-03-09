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
    base_lr_warmup_steps: u64,
    base_lr_final_scale: f32,
    base_grad_clip_norm: f32,
    base_model_dim: usize,
    base_mlp_dim: usize,
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
        base_learning_rate: 0.03,
        base_lr_warmup_steps: 200,
        base_lr_final_scale: 0.2,
        base_grad_clip_norm: 1.0,
        base_model_dim: 32,
        base_mlp_dim: 64,
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
            "--base-lr-warmup-steps" => {
                args.base_lr_warmup_steps = it.next().ok_or("missing value")?.parse()?
            }
            "--base-lr-final-scale" => {
                args.base_lr_final_scale = it.next().ok_or("missing value")?.parse()?
            }
            "--base-grad-clip-norm" => {
                args.base_grad_clip_norm = it.next().ok_or("missing value")?.parse()?
            }
            "--base-model-dim" => {
                args.base_model_dim = it.next().ok_or("missing value")?.parse()?
            }
            "--base-mlp-dim" => args.base_mlp_dim = it.next().ok_or("missing value")?.parse()?,
            "--seed" => args.seed = it.next().ok_or("missing value")?.parse()?,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    if args.base_model_dim == 0 || args.base_mlp_dim == 0 {
        return Err("base model dimensions must be > 0".into());
    }
    if !(0.0..=1.0).contains(&args.base_lr_final_scale) {
        return Err("base_lr_final_scale must be in [0, 1]".into());
    }
    if args.base_grad_clip_norm <= 0.0 {
        return Err("base_grad_clip_norm must be > 0".into());
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
    println!("  --base-learning-rate <f>     default: 0.03");
    println!("  --base-lr-warmup-steps <n>   default: 200");
    println!("  --base-lr-final-scale <f>    default: 0.2");
    println!("  --base-grad-clip-norm <f>    default: 1.0");
    println!("  --base-model-dim <n>         default: 32");
    println!("  --base-mlp-dim <n>           default: 64");
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
        let lr_warmup_steps = mutate_u64(
            args.base_lr_warmup_steps,
            s.rotate_left(7),
            &[0.5, 1.0, 2.0],
        );
        let lr_final_scale = mutate_range_f32(args.base_lr_final_scale, s.rotate_left(11), 0.05);
        let grad_clip_norm = mutate_range_f32(args.base_grad_clip_norm, s.rotate_left(19), 0.2);
        let model_dim =
            mutate_multiple_of_8(args.base_model_dim, s.rotate_left(23), &[0.5, 1.0, 1.5]);
        let mlp_dim = mutate_multiple_of_8(args.base_mlp_dim, s.rotate_left(31), &[0.5, 1.0, 1.5]);

        let description = format!(
            "auto exp={} batch_size={} seq_len={} lr={:.4} warmup={} final_scale={:.3} clip={:.3} model_dim={} mlp_dim={} seed={}",
            exp_idx + 1,
            batch_size,
            seq_len,
            learning_rate,
            lr_warmup_steps,
            lr_final_scale,
            grad_clip_norm,
            model_dim,
            mlp_dim,
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
            .arg("--lr-warmup-steps")
            .arg(lr_warmup_steps.to_string())
            .arg("--lr-final-scale")
            .arg(format!("{lr_final_scale:.6}"))
            .arg("--grad-clip-norm")
            .arg(format!("{grad_clip_norm:.6}"))
            .arg("--model-dim")
            .arg(model_dim.to_string())
            .arg("--mlp-dim")
            .arg(mlp_dim.to_string())
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

fn mutate_u64(base: u64, state: u64, factors: &[f32]) -> u64 {
    let idx = (state as usize) % factors.len();
    (base as f32 * factors[idx]).round().max(0.0) as u64
}

fn mutate_range_f32(base: f32, state: u64, delta: f32) -> f32 {
    let bucket = (state % 5) as i64 - 2;
    (base + bucket as f32 * delta).clamp(1e-6, 1.0)
}

fn mutate_multiple_of_8(base: usize, state: u64, factors: &[f32]) -> usize {
    let idx = (state as usize) % factors.len();
    let raw = (base as f32 * factors[idx]).round().max(8.0) as usize;
    (raw / 8).max(1) * 8
}
