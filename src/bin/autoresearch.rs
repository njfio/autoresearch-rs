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
    base_lr_min_scale: f32,
    base_grad_clip_norm: f32,
    base_n_layers: usize,
    base_n_heads: usize,
    base_d_model: usize,
    base_d_ff: usize,
    base_dropout: f32,
    base_weight_decay: f32,
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
        base_lr_min_scale: 0.2,
        base_grad_clip_norm: 1.0,
        base_n_layers: 2,
        base_n_heads: 4,
        base_d_model: 64,
        base_d_ff: 256,
        base_dropout: 0.0,
        base_weight_decay: 0.1,
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
            "--base-lr-final-scale" | "--base-lr-min-scale" => {
                args.base_lr_min_scale = it.next().ok_or("missing value")?.parse()?
            }
            "--base-grad-clip-norm" => {
                args.base_grad_clip_norm = it.next().ok_or("missing value")?.parse()?
            }
            "--base-model-dim" | "--base-d-model" => {
                args.base_d_model = it.next().ok_or("missing value")?.parse()?
            }
            "--base-mlp-dim" | "--base-d-ff" => {
                args.base_d_ff = it.next().ok_or("missing value")?.parse()?
            }
            "--base-n-layers" => args.base_n_layers = it.next().ok_or("missing value")?.parse()?,
            "--base-n-heads" => args.base_n_heads = it.next().ok_or("missing value")?.parse()?,
            "--base-dropout" => args.base_dropout = it.next().ok_or("missing value")?.parse()?,
            "--base-weight-decay" => {
                args.base_weight_decay = it.next().ok_or("missing value")?.parse()?
            }
            "--seed" => args.seed = it.next().ok_or("missing value")?.parse()?,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    if args.base_n_layers == 0 || args.base_n_heads == 0 {
        return Err("base_n_layers and base_n_heads must be > 0".into());
    }
    if args.base_d_model == 0 || args.base_d_ff == 0 {
        return Err("base model dimensions must be > 0".into());
    }
    if args.base_d_model % args.base_n_heads != 0 {
        return Err("base_d_model must be divisible by base_n_heads".into());
    }
    if !(0.0..=1.0).contains(&args.base_lr_min_scale) {
        return Err("base_lr_min_scale must be in [0, 1]".into());
    }
    if args.base_grad_clip_norm <= 0.0 {
        return Err("base_grad_clip_norm must be > 0".into());
    }
    if !(0.0..1.0).contains(&args.base_dropout) {
        return Err("base_dropout must be in [0, 1)".into());
    }
    if args.base_weight_decay < 0.0 {
        return Err("base_weight_decay must be >= 0".into());
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
    println!("  --base-lr-min-scale <f>      default: 0.2");
    println!("  --base-lr-final-scale <f>    alias for --base-lr-min-scale");
    println!("  --base-grad-clip-norm <f>    default: 1.0");
    println!("  --base-n-layers <n>          default: 2");
    println!("  --base-n-heads <n>           default: 4");
    println!("  --base-d-model <n>           default: 64");
    println!("  --base-d-ff <n>              default: 256");
    println!("  --base-model-dim <n>         alias for --base-d-model");
    println!("  --base-mlp-dim <n>           alias for --base-d-ff");
    println!("  --base-dropout <f>           default: 0.0");
    println!("  --base-weight-decay <f>      default: 0.1");
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
        let lr_min_scale = mutate_range_f32(args.base_lr_min_scale, s.rotate_left(11), 0.05);
        let grad_clip_norm = mutate_range_f32(args.base_grad_clip_norm, s.rotate_left(19), 0.2);
        let n_layers = mutate_usize(args.base_n_layers, s.rotate_left(3), &[0.5, 1.0, 1.5]).max(1);
        let n_heads = mutate_usize(args.base_n_heads, s.rotate_left(5), &[0.5, 1.0, 2.0]).max(1);
        let mut d_model =
            mutate_multiple_of_8(args.base_d_model, s.rotate_left(23), &[0.5, 1.0, 1.5]);
        if d_model % n_heads != 0 {
            d_model = ((d_model / n_heads).max(1)) * n_heads;
        }
        let d_ff = mutate_multiple_of_8(args.base_d_ff, s.rotate_left(31), &[0.5, 1.0, 1.5]);
        let dropout = mutate_range_f32(args.base_dropout, s.rotate_left(37), 0.05).clamp(0.0, 0.8);
        let weight_decay = mutate_range_f32(args.base_weight_decay, s.rotate_left(41), 0.05);

        let description = format!(
            "auto exp={} batch_size={} seq_len={} lr={:.4} warmup={} min_scale={:.3} clip={:.3} n_layers={} n_heads={} d_model={} d_ff={} dropout={:.3} wd={:.3} seed={}",
            exp_idx + 1,
            batch_size,
            seq_len,
            learning_rate,
            lr_warmup_steps,
            lr_min_scale,
            grad_clip_norm,
            n_layers,
            n_heads,
            d_model,
            d_ff,
            dropout,
            weight_decay,
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
            .arg("--lr-min-scale")
            .arg(format!("{lr_min_scale:.6}"))
            .arg("--grad-clip-norm")
            .arg(format!("{grad_clip_norm:.6}"))
            .arg("--n-layers")
            .arg(n_layers.to_string())
            .arg("--n-heads")
            .arg(n_heads.to_string())
            .arg("--d-model")
            .arg(d_model.to_string())
            .arg("--d-ff")
            .arg(d_ff.to_string())
            .arg("--dropout")
            .arg(format!("{dropout:.6}"))
            .arg("--weight-decay")
            .arg(format!("{weight_decay:.6}"))
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
