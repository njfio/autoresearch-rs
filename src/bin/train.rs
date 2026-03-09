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
use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::loss;
use candle_nn::ops;
use candle_nn::{
    embedding, layer_norm, linear, AdamW, Embedding, LayerNorm, Linear, Module, Optimizer,
    ParamsAdamW, VarBuilder, VarMap,
};

#[derive(Debug, Clone)]
struct Args {
    artifacts_dir: PathBuf,
    runs_dir: PathBuf,
    time_budget_seconds: u64,
    batch_size: usize,
    seq_len: usize,
    learning_rate: f64,
    lr_warmup_steps: u64,
    lr_min_scale: f64,
    grad_clip_norm: f64,
    n_layers: usize,
    n_heads: usize,
    d_model: usize,
    d_ff: usize,
    dropout: f64,
    weight_decay: f64,
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
        learning_rate: 3e-3,
        lr_warmup_steps: 200,
        lr_min_scale: 0.2,
        grad_clip_norm: 1.0,
        n_layers: 2,
        n_heads: 4,
        d_model: 64,
        d_ff: 256,
        dropout: 0.0,
        weight_decay: 0.1,
        eval_interval: 250,
        eval_batches: 64,
        seed: 1337,
        description: "tiny GPT (candle, adamw, cosine-warmup schedule)".to_string(),
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
            "--lr-warmup-steps" => {
                args.lr_warmup_steps = it.next().ok_or("missing value")?.parse()?
            }
            "--lr-final-scale" | "--lr-min-scale" => {
                args.lr_min_scale = it.next().ok_or("missing value")?.parse()?
            }
            "--grad-clip-norm" => {
                args.grad_clip_norm = it.next().ok_or("missing value")?.parse()?
            }
            "--model-dim" | "--d-model" => {
                args.d_model = it.next().ok_or("missing value")?.parse()?
            }
            "--mlp-dim" | "--d-ff" => args.d_ff = it.next().ok_or("missing value")?.parse()?,
            "--n-layers" => args.n_layers = it.next().ok_or("missing value")?.parse()?,
            "--n-heads" => args.n_heads = it.next().ok_or("missing value")?.parse()?,
            "--dropout" => args.dropout = it.next().ok_or("missing value")?.parse()?,
            "--weight-decay" => args.weight_decay = it.next().ok_or("missing value")?.parse()?,
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

    if args.n_layers == 0 {
        return Err("n_layers must be > 0".into());
    }
    if args.n_heads == 0 {
        return Err("n_heads must be > 0".into());
    }
    if args.d_model == 0 || args.d_ff == 0 {
        return Err("d_model and d_ff must be > 0".into());
    }
    if args.d_model % args.n_heads != 0 {
        return Err("d_model must be divisible by n_heads".into());
    }
    if !(0.0..=1.0).contains(&args.lr_min_scale) {
        return Err("lr_min_scale must be in [0, 1]".into());
    }
    if !(0.0..1.0).contains(&args.dropout) {
        return Err("dropout must be in [0, 1)".into());
    }
    if args.weight_decay < 0.0 {
        return Err("weight_decay must be >= 0".into());
    }
    if args.grad_clip_norm <= 0.0 {
        return Err("grad_clip_norm must be > 0".into());
    }

    Ok(args)
}

fn print_help() {
    println!("train usage:");
    println!("  cargo run --bin train -- [options]");
    println!("options:");
    println!("  --artifacts-dir <path>      default: artifacts");
    println!("  --runs-dir <path>           default: runs");
    println!("  --time-budget-seconds <s>   default: 300");
    println!("  --batch-size <n>            default: 32");
    println!("  --seq-len <n>               default: 64");
    println!("  --learning-rate <f>         default: 0.003");
    println!("  --lr-warmup-steps <n>       default: 200");
    println!("  --lr-min-scale <f>          default: 0.2");
    println!("  --lr-final-scale <f>        alias for --lr-min-scale");
    println!("  --grad-clip-norm <f>        default: 1.0");
    println!("  --n-layers <n>              default: 2");
    println!("  --n-heads <n>               default: 4");
    println!("  --d-model <n>               default: 64");
    println!("  --d-ff <n>                  default: 256");
    println!("  --model-dim <n>             alias for --d-model");
    println!("  --mlp-dim <n>               alias for --d-ff");
    println!("  --dropout <f>               default: 0.0");
    println!("  --weight-decay <f>          default: 0.1");
    println!("  --eval-interval <n>         default: 250");
    println!("  --eval-batches <n>          default: 64");
    println!("  --seed <u64>                default: 1337");
    println!("  --description <text>");
}

#[derive(Debug)]
struct TransformerBlock {
    ln1: LayerNorm,
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    attn_out: Linear,
    ln2: LayerNorm,
    ff1: Linear,
    ff2: Linear,
}

#[derive(Debug)]
struct TinyGpt {
    tok_emb: Embedding,
    pos_emb: Tensor,
    blocks: Vec<TransformerBlock>,
    ln_f: LayerNorm,
    lm_head: Linear,
    n_heads: usize,
    d_model: usize,
    dropout: f64,
    device: Device,
}

impl TinyGpt {
    fn new(
        vb: VarBuilder,
        vocab_size: usize,
        seq_len: usize,
        n_layers: usize,
        n_heads: usize,
        d_model: usize,
        d_ff: usize,
        dropout: f64,
        device: &Device,
    ) -> candle_core::Result<Self> {
        let tok_emb = embedding(vocab_size, d_model, vb.pp("tok_emb"))?;
        let pos_emb = vb.get_with_hints(
            (seq_len, d_model),
            "pos_emb",
            candle_nn::Init::Randn {
                mean: 0.0,
                stdev: 0.02,
            },
        )?;

        let mut blocks = Vec::with_capacity(n_layers);
        for i in 0..n_layers {
            let b = vb.pp(format!("blocks.{i}"));
            blocks.push(TransformerBlock {
                ln1: layer_norm(d_model, 1e-5, b.pp("ln1"))?,
                q_proj: linear(d_model, d_model, b.pp("q"))?,
                k_proj: linear(d_model, d_model, b.pp("k"))?,
                v_proj: linear(d_model, d_model, b.pp("v"))?,
                attn_out: linear(d_model, d_model, b.pp("attn_out"))?,
                ln2: layer_norm(d_model, 1e-5, b.pp("ln2"))?,
                ff1: linear(d_model, d_ff, b.pp("ff1"))?,
                ff2: linear(d_ff, d_model, b.pp("ff2"))?,
            });
        }

        Ok(Self {
            tok_emb,
            pos_emb,
            blocks,
            ln_f: layer_norm(d_model, 1e-5, vb.pp("ln_f"))?,
            lm_head: linear(d_model, vocab_size, vb.pp("lm_head"))?,
            n_heads,
            d_model,
            dropout,
            device: device.clone(),
        })
    }

    fn apply_dropout(&self, x: &Tensor, train: bool) -> candle_core::Result<Tensor> {
        if !train || self.dropout <= 0.0 {
            Ok(x.clone())
        } else {
            ops::dropout(x, self.dropout as f32)
        }
    }

    fn causal_mask(&self, t: usize) -> candle_core::Result<Tensor> {
        let mut mask = vec![0f32; t * t];
        for i in 0..t {
            for j in (i + 1)..t {
                mask[i * t + j] = -1e9;
            }
        }
        Tensor::from_vec(mask, (1, 1, t, t), &self.device)
    }

    fn forward(&self, tokens: &Tensor, train: bool) -> candle_core::Result<Tensor> {
        let (b, t) = tokens.dims2()?;
        let mut x = self.tok_emb.forward(tokens)?;
        let pos = self
            .pos_emb
            .i(0..t)?
            .unsqueeze(0)?
            .broadcast_as(x.shape())?;
        x = (x + pos)?;

        let head_dim = self.d_model / self.n_heads;
        let scale = 1.0f64 / (head_dim as f64).sqrt();
        let mask = self.causal_mask(t)?;

        for block in &self.blocks {
            let x_norm = block.ln1.forward(&x)?;
            let q = block
                .q_proj
                .forward(&x_norm)?
                .reshape((b, t, self.n_heads, head_dim))?
                .transpose(1, 2)?
                .contiguous()?;
            let k = block
                .k_proj
                .forward(&x_norm)?
                .reshape((b, t, self.n_heads, head_dim))?
                .transpose(1, 2)?
                .contiguous()?;
            let v = block
                .v_proj
                .forward(&x_norm)?
                .reshape((b, t, self.n_heads, head_dim))?
                .transpose(1, 2)?
                .contiguous()?;

            let att = q
                .matmul(&k.transpose(2, 3)?)?
                .affine(scale, 0.0)?
                .broadcast_add(&mask)?;
            let att = self.apply_dropout(&ops::softmax_last_dim(&att)?, train)?;
            let y = att
                .matmul(&v)?
                .transpose(1, 2)?
                .reshape((b, t, self.d_model))?;
            let y = self.apply_dropout(&block.attn_out.forward(&y)?, train)?;
            x = (x + y)?;

            let ff = block
                .ff2
                .forward(&block.ff1.forward(&block.ln2.forward(&x)?)?.relu()?)?;
            x = (x + self.apply_dropout(&ff, train)?)?;
        }

        self.lm_head.forward(&self.ln_f.forward(&x)?)
    }
}

fn sample_batch(
    data: &[u16],
    batch_size: usize,
    seq_len: usize,
    rng: &mut SimpleRng,
) -> (Vec<u32>, Vec<u32>) {
    let mut x = vec![0u32; batch_size * seq_len];
    let mut y = vec![0u32; batch_size * seq_len];
    for b in 0..batch_size {
        let start = rng.gen_range(data.len() - seq_len - 1);
        for t in 0..seq_len {
            x[b * seq_len + t] = data[start + t] as u32;
            y[b * seq_len + t] = data[start + t + 1] as u32;
        }
    }
    (x, y)
}

fn learning_rate_at(
    base_lr: f64,
    lr_warmup_steps: u64,
    lr_min_scale: f64,
    step: u64,
    elapsed: Duration,
    budget: Duration,
) -> f64 {
    if step <= lr_warmup_steps && lr_warmup_steps > 0 {
        return base_lr * (step as f64 / lr_warmup_steps as f64);
    }
    let progress = if budget.is_zero() {
        1.0
    } else {
        (elapsed.as_secs_f64() / budget.as_secs_f64()).clamp(0.0, 1.0)
    };
    let cosine = 0.5 * (1.0 + (std::f64::consts::PI * progress).cos());
    let min_lr = base_lr * lr_min_scale;
    min_lr + (base_lr - min_lr) * cosine
}

fn eval_val_bpb(
    model: &TinyGpt,
    val_tokens: &[u16],
    token_bytes: &[u32],
    batch_size: usize,
    seq_len: usize,
    eval_batches: usize,
    deadline: Option<Instant>,
    rng: &mut SimpleRng,
) -> AppResult<f64> {
    let mut total_nats = 0.0f64;
    let mut total_bytes = 0u64;
    let mut total_targets = 0usize;

    for _ in 0..eval_batches {
        if let Some(dl) = deadline {
            if Instant::now() >= dl && total_targets > 0 {
                break;
            }
        }

        let (x, y) = sample_batch(val_tokens, batch_size, seq_len, rng);
        let x = Tensor::from_vec(x, (batch_size, seq_len), &model.device)?;
        let y = Tensor::from_vec(y, (batch_size, seq_len), &model.device)?;

        let logits = model.forward(&x, false)?;
        let logits = logits.reshape((batch_size * seq_len, logits.dim(2)?))?;
        let y_flat = y.reshape(batch_size * seq_len)?;
        let mean_loss = loss::cross_entropy(&logits, &y_flat)?;
        let mean_loss = mean_loss.to_dtype(DType::F64)?.to_scalar::<f64>()?;

        let targets: Vec<u32> = y_flat.to_vec1()?;
        for id in targets {
            total_bytes += token_bytes[id as usize] as u64;
        }
        total_targets += batch_size * seq_len;
        total_nats += mean_loss * (batch_size * seq_len) as f64;
    }

    if total_targets == 0 || total_bytes == 0 {
        return Err("validation produced zero targets/bytes".into());
    }
    Ok(nats_to_bpb(total_nats, total_bytes))
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
            ("lr_warmup_steps", args.lr_warmup_steps.to_string()),
            ("lr_final_scale", args.lr_min_scale.to_string()),
            ("lr_min_scale", args.lr_min_scale.to_string()),
            ("grad_clip_norm", args.grad_clip_norm.to_string()),
            ("model_dim", args.d_model.to_string()),
            ("mlp_dim", args.d_ff.to_string()),
            ("d_model", args.d_model.to_string()),
            ("d_ff", args.d_ff.to_string()),
            ("n_layers", args.n_layers.to_string()),
            ("n_heads", args.n_heads.to_string()),
            ("dropout", args.dropout.to_string()),
            ("weight_decay", args.weight_decay.to_string()),
            ("eval_interval", args.eval_interval.to_string()),
            ("eval_batches", args.eval_batches.to_string()),
            ("seed", args.seed.to_string()),
            ("description", args.description.clone()),
        ],
    )?;

    let metrics_path = run_dir.join("metrics.tsv");
    fs::write(&metrics_path, "step\telapsed_s\ttrain_nll_nats\tval_bpb\n")?;

    let device = Device::Cpu;
    let varmap = VarMap::new();
    let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
    let model = TinyGpt::new(
        vb,
        tokenizer.vocab.len(),
        args.seq_len,
        args.n_layers,
        args.n_heads,
        args.d_model,
        args.d_ff,
        args.dropout,
        &device,
    )?;

    let adamw = ParamsAdamW {
        lr: args.learning_rate,
        weight_decay: args.weight_decay,
        ..Default::default()
    };
    let mut opt = AdamW::new(varmap.all_vars(), adamw)?;

    let t0_total = Instant::now();
    let t0_train = Instant::now();
    let budget = Duration::from_secs(args.time_budget_seconds);
    let deadline = t0_train + budget;

    let mut rng = SimpleRng::seed(args.seed);
    let mut step: u64 = 0;
    let mut total_tokens: u64 = 0;
    let mut last_val_bpb = f64::INFINITY;

    while Instant::now() < deadline {
        step += 1;
        let lr = learning_rate_at(
            args.learning_rate,
            args.lr_warmup_steps,
            args.lr_min_scale,
            step,
            t0_train.elapsed(),
            budget,
        );
        opt.set_learning_rate(lr);

        let (x, y) = sample_batch(&train_tokens, args.batch_size, args.seq_len, &mut rng);
        let x = Tensor::from_vec(x, (args.batch_size, args.seq_len), &device)?;
        let y = Tensor::from_vec(y, (args.batch_size, args.seq_len), &device)?;

        let logits = model.forward(&x, true)?;
        let logits = logits.reshape((args.batch_size * args.seq_len, logits.dim(2)?))?;
        let y_flat = y.reshape(args.batch_size * args.seq_len)?;
        let loss = loss::cross_entropy(&logits, &y_flat)?;
        let train_nll_nats = loss.to_dtype(DType::F64)?.to_scalar::<f64>()?;

        opt.backward_step(&loss)?;
        total_tokens += (args.batch_size * args.seq_len) as u64;

        if step % args.eval_interval == 0 {
            last_val_bpb = eval_val_bpb(
                &model,
                &val_tokens,
                &tokenizer.token_bytes,
                args.batch_size,
                args.seq_len,
                args.eval_batches,
                Some(deadline),
                &mut rng,
            )?;
            let line = format!(
                "{}\t{:.1}\t{:.6}\t{:.6}\n",
                step,
                t0_train.elapsed().as_secs_f64(),
                train_nll_nats,
                last_val_bpb
            );
            fs::OpenOptions::new()
                .append(true)
                .open(&metrics_path)?
                .write_all(line.as_bytes())?;
        }
    }

    if !last_val_bpb.is_finite() {
        last_val_bpb = eval_val_bpb(
            &model,
            &val_tokens,
            &tokenizer.token_bytes,
            args.batch_size,
            args.seq_len,
            args.eval_batches,
            Some(deadline),
            &mut rng,
        )?;
    }

    let training_seconds = t0_train.elapsed().as_secs_f64();
    let total_seconds = t0_total.elapsed().as_secs_f64();
    let improved = prev_best.map(|b| last_val_bpb < b).unwrap_or(true);
    let status = if improved { "keep" } else { "discard" };

    let checkpoint_path = run_dir.join("checkpoint.bin");
    varmap.save(&checkpoint_path)?;

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
