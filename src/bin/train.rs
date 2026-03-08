use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use autoresearch_rs::{
    append_results_row, current_best_bpb, ensure_dir, init_results_tsv, nats_to_bpb,
    read_tokenizer, read_u16_tokens, timestamp_run_id, write_kv, AppResult, SimpleRng,
    DEFAULT_BATCH_SIZE, DEFAULT_SEQ_LEN, DEFAULT_TIME_BUDGET_SECONDS,
};

#[derive(Debug, Clone)]
struct Args {
    artifacts_dir: PathBuf,
    runs_dir: PathBuf,
    time_budget_seconds: u64,
    batch_size: usize,
    seq_len: usize,
    learning_rate: f32,
    eval_interval: u64,
    eval_batches: usize,
    seed: u64,
    description: String,
}

fn parse_args() -> AppResult<Args> {
    let mut args = Args {
        artifacts_dir: PathBuf::from("artifacts"),
        runs_dir: PathBuf::from("runs"),
        time_budget_seconds: DEFAULT_TIME_BUDGET_SECONDS,
        batch_size: DEFAULT_BATCH_SIZE,
        seq_len: DEFAULT_SEQ_LEN,
        learning_rate: 1.0,
        eval_interval: 250,
        eval_batches: 64,
        seed: 1337,
        description: "baseline bigram autoregressive model".to_string(),
    };

    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--artifacts-dir" => {
                args.artifacts_dir = PathBuf::from(it.next().ok_or("missing value")?)
            }
            "--runs-dir" => args.runs_dir = PathBuf::from(it.next().ok_or("missing value")?),
            "--time-budget-seconds" => {
                args.time_budget_seconds = it.next().ok_or("missing value")?.parse()?
            }
            "--batch-size" => args.batch_size = it.next().ok_or("missing value")?.parse()?,
            "--seq-len" => args.seq_len = it.next().ok_or("missing value")?.parse()?,
            "--learning-rate" => args.learning_rate = it.next().ok_or("missing value")?.parse()?,
            "--eval-interval" => args.eval_interval = it.next().ok_or("missing value")?.parse()?,
            "--eval-batches" => args.eval_batches = it.next().ok_or("missing value")?.parse()?,
            "--seed" => args.seed = it.next().ok_or("missing value")?.parse()?,
            "--description" => args.description = it.next().ok_or("missing value")?,
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
    println!("train usage:");
    println!("  cargo run --bin train -- [options]");
}

#[derive(Debug, Clone)]
struct BigramLM {
    vocab_size: usize,
    weights: Vec<f32>,
}

impl BigramLM {
    fn new(vocab_size: usize, rng: &mut SimpleRng) -> Self {
        let mut weights = vec![0.0_f32; vocab_size * vocab_size];
        for w in &mut weights {
            *w = rng.gen_f32_range(-0.01, 0.01);
        }
        Self {
            vocab_size,
            weights,
        }
    }

    fn row(&self, token: u16) -> &[f32] {
        let start = token as usize * self.vocab_size;
        &self.weights[start..start + self.vocab_size]
    }

    fn train_step(
        &mut self,
        data: &[u16],
        batch_size: usize,
        seq_len: usize,
        lr: f32,
        rng: &mut SimpleRng,
    ) -> AppResult<f64> {
        if data.len() <= seq_len + 1 {
            return Err(format!("dataset too short for seq_len={seq_len}").into());
        }

        let mut grad = vec![0.0_f32; self.weights.len()];
        let mut total_loss = 0.0_f64;
        let mut ntokens = 0usize;

        for _ in 0..batch_size {
            let start = rng.gen_range(data.len() - seq_len - 1);
            for t in 0..seq_len {
                let x = data[start + t] as usize;
                let y = data[start + t + 1] as usize;
                let row_start = x * self.vocab_size;
                let logits = &self.weights[row_start..row_start + self.vocab_size];

                let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let mut exp_sum = 0.0_f64;
                let mut probs = vec![0.0_f64; self.vocab_size];
                for (j, &logit) in logits.iter().enumerate() {
                    let p = ((logit - max_logit) as f64).exp();
                    probs[j] = p;
                    exp_sum += p;
                }
                for p in &mut probs {
                    *p /= exp_sum;
                }

                let target_prob = probs[y].max(1e-12);
                total_loss += -target_prob.ln();

                for (j, p) in probs.iter().enumerate() {
                    grad[row_start + j] += *p as f32;
                }
                grad[row_start + y] -= 1.0;
                ntokens += 1;
            }
        }

        let scale = lr / ntokens as f32;
        for (w, g) in self.weights.iter_mut().zip(grad.iter()) {
            *w -= scale * *g;
        }

        Ok(total_loss / ntokens as f64)
    }

    fn evaluate_bpb(
        &self,
        data: &[u16],
        token_bytes: &[u32],
        batch_size: usize,
        seq_len: usize,
        eval_batches: usize,
        rng: &mut SimpleRng,
    ) -> AppResult<f64> {
        if data.len() <= seq_len + 1 {
            return Err(format!("dataset too short for eval seq_len={seq_len}").into());
        }

        let mut total_nats = 0.0_f64;
        let mut total_bytes = 0_u64;

        for _ in 0..eval_batches {
            for _ in 0..batch_size {
                let start = rng.gen_range(data.len() - seq_len - 1);
                for t in 0..seq_len {
                    let x = data[start + t];
                    let y = data[start + t + 1];
                    let logits = self.row(x);

                    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                    let mut exp_sum = 0.0_f64;
                    let mut target_exp = 0.0_f64;
                    for (j, &logit) in logits.iter().enumerate() {
                        let ex = ((logit - max_logit) as f64).exp();
                        if j == y as usize {
                            target_exp = ex;
                        }
                        exp_sum += ex;
                    }

                    let p = (target_exp / exp_sum).max(1e-12);
                    total_nats += -p.ln();
                    total_bytes += token_bytes[y as usize] as u64;
                }
            }
        }

        if total_bytes == 0 {
            return Err("total_bytes was zero during evaluation".into());
        }
        Ok(nats_to_bpb(total_nats, total_bytes))
    }

    fn save_checkpoint(&self, path: &PathBuf) -> AppResult<()> {
        let mut buf = Vec::with_capacity(self.weights.len() * 4);
        for v in &self.weights {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        fs::write(path, buf)?;
        Ok(())
    }
}

fn main() -> AppResult<()> {
    let args = parse_args()?;

    ensure_dir(&args.runs_dir)?;
    let tokenizer = read_tokenizer(&args.artifacts_dir.join("tokenizer.txt"))
        .map_err(|_| "missing artifacts; run `cargo run --bin prepare` first")?;
    let train_tokens = read_u16_tokens(&args.artifacts_dir.join("train_tokens.bin"))?;
    let val_tokens = read_u16_tokens(&args.artifacts_dir.join("val_tokens.bin"))?;

    if train_tokens.len() <= args.seq_len + 1 || val_tokens.len() <= args.seq_len + 1 {
        return Err(format!("dataset too small for seq_len={}", args.seq_len).into());
    }

    let run_id = timestamp_run_id();
    let run_dir = args.runs_dir.join(&run_id);
    ensure_dir(&run_dir)?;

    let results_tsv = init_results_tsv(&args.runs_dir)?;
    let prev_best = current_best_bpb(&results_tsv)?;

    write_kv(
        &run_dir.join("config.txt"),
        &[
            ("artifacts_dir", args.artifacts_dir.display().to_string()),
            ("time_budget_seconds", args.time_budget_seconds.to_string()),
            ("batch_size", args.batch_size.to_string()),
            ("seq_len", args.seq_len.to_string()),
            ("learning_rate", args.learning_rate.to_string()),
            ("eval_interval", args.eval_interval.to_string()),
            ("eval_batches", args.eval_batches.to_string()),
            ("seed", args.seed.to_string()),
            ("description", args.description.clone()),
        ],
    )?;

    let metrics_path = run_dir.join("metrics.tsv");
    fs::write(&metrics_path, "step\telapsed_s\ttrain_nll_nats\tval_bpb\n")?;

    let mut rng = SimpleRng::seed(args.seed);
    let mut model = BigramLM::new(tokenizer.vocab.len(), &mut rng);

    let t0_total = Instant::now();
    let t0_train = Instant::now();
    let budget = Duration::from_secs(args.time_budget_seconds);

    let mut step: u64 = 0;
    let mut total_tokens: u64 = 0;
    let mut last_val_bpb = f64::INFINITY;

    while t0_train.elapsed() < budget {
        step += 1;
        let train_loss = model.train_step(
            &train_tokens,
            args.batch_size,
            args.seq_len,
            args.learning_rate,
            &mut rng,
        )?;
        total_tokens += (args.batch_size * args.seq_len) as u64;

        if step % args.eval_interval == 0 {
            last_val_bpb = model.evaluate_bpb(
                &val_tokens,
                &tokenizer.token_bytes,
                args.batch_size,
                args.seq_len,
                args.eval_batches,
                &mut rng,
            )?;
            let line = format!(
                "{}\t{:.1}\t{:.6}\t{:.6}\n",
                step,
                t0_train.elapsed().as_secs_f64(),
                train_loss,
                last_val_bpb
            );
            fs::OpenOptions::new()
                .append(true)
                .open(&metrics_path)?
                .write_all(line.as_bytes())?;
        }
    }

    if !last_val_bpb.is_finite() {
        last_val_bpb = model.evaluate_bpb(
            &val_tokens,
            &tokenizer.token_bytes,
            args.batch_size,
            args.seq_len,
            args.eval_batches,
            &mut rng,
        )?;
    }

    let training_seconds = t0_train.elapsed().as_secs_f64();
    let total_seconds = t0_total.elapsed().as_secs_f64();
    let improved = prev_best.map(|b| last_val_bpb < b).unwrap_or(true);
    let status = if improved { "keep" } else { "discard" };

    let checkpoint_path = run_dir.join("checkpoint.bin");
    model.save_checkpoint(&checkpoint_path)?;

    write_kv(
        &run_dir.join("summary.txt"),
        &[
            ("run_id", run_id.clone()),
            ("val_bpb", format!("{:.6}", last_val_bpb)),
            ("training_seconds", format!("{:.1}", training_seconds)),
            ("total_seconds", format!("{:.1}", total_seconds)),
            ("num_steps", step.to_string()),
            ("total_tokens", total_tokens.to_string()),
            ("vocab_size", tokenizer.vocab.len().to_string()),
            ("seq_len", args.seq_len.to_string()),
            ("batch_size", args.batch_size.to_string()),
            ("status", status.to_string()),
            ("checkpoint", checkpoint_path.display().to_string()),
        ],
    )?;

    write_kv(
        &args.runs_dir.join("latest.txt"),
        &[
            ("run_id", run_id.clone()),
            ("val_bpb", format!("{:.6}", last_val_bpb)),
            ("status", status.to_string()),
            ("checkpoint", checkpoint_path.display().to_string()),
        ],
    )?;

    if improved {
        write_kv(
            &args.runs_dir.join("best.txt"),
            &[
                ("run_id", run_id.clone()),
                ("val_bpb", format!("{:.6}", last_val_bpb)),
                ("status", status.to_string()),
                ("checkpoint", checkpoint_path.display().to_string()),
            ],
        )?;
    }

    append_results_row(
        &results_tsv,
        &run_id,
        last_val_bpb,
        training_seconds,
        status,
        &args.description,
    )?;

    println!("---");
    println!("run_id:            {}", run_id);
    println!("val_bpb:           {:.6}", last_val_bpb);
    println!("training_seconds:  {:.1}", training_seconds);
    println!("total_seconds:     {:.1}", total_seconds);
    println!("num_steps:         {}", step);
    println!("total_tokens:      {}", total_tokens);
    println!("vocab_size:        {}", tokenizer.vocab.len());
    println!("status:            {}", status);

    Ok(())
}
