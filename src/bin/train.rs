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
        learning_rate: 0.03,
        eval_interval: 250,
        eval_batches: 64,
        seed: 1337,
        description: "baseline tiny causal transformer model".to_string(),
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

const MODEL_DIM: usize = 32;
const MLP_DIM: usize = 64;
const LN_EPS: f32 = 1e-5;

fn relu(x: f32) -> f32 {
    if x > 0.0 {
        x
    } else {
        0.0
    }
}

fn stable_softmax(logits: &[f32], out: &mut [f32]) {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0_f32;
    for (i, v) in logits.iter().enumerate() {
        let e = (*v - max_logit).exp();
        out[i] = e;
        sum += e;
    }
    let inv = 1.0_f32 / sum.max(1e-12);
    for p in out.iter_mut() {
        *p *= inv;
    }
}

fn layer_norm_forward(
    input: &[f32],
    gain: &[f32],
    bias: &[f32],
    out: &mut [f32],
    xhat: &mut [f32],
) -> f32 {
    let n = input.len() as f32;
    let mean = input.iter().sum::<f32>() / n;
    let var = input
        .iter()
        .map(|x| {
            let d = *x - mean;
            d * d
        })
        .sum::<f32>()
        / n;
    let inv_std = 1.0 / (var + LN_EPS).sqrt();
    for i in 0..input.len() {
        xhat[i] = (input[i] - mean) * inv_std;
        out[i] = xhat[i] * gain[i] + bias[i];
    }
    inv_std
}

fn layer_norm_backward(
    dout: &[f32],
    xhat: &[f32],
    inv_std: f32,
    gain: &[f32],
    grad_gain: &mut [f32],
    grad_bias: &mut [f32],
    dx: &mut [f32],
) {
    let n = dout.len() as f32;
    let mut sum_dxhat = 0.0_f32;
    let mut sum_dxhat_xhat = 0.0_f32;
    for i in 0..dout.len() {
        grad_gain[i] += dout[i] * xhat[i];
        grad_bias[i] += dout[i];
        let dxhat = dout[i] * gain[i];
        sum_dxhat += dxhat;
        sum_dxhat_xhat += dxhat * xhat[i];
    }
    for i in 0..dout.len() {
        let dxhat = dout[i] * gain[i];
        dx[i] += (inv_std / n) * (n * dxhat - sum_dxhat - xhat[i] * sum_dxhat_xhat);
    }
}

fn linear_forward(input: &[f32], weight: &[f32], bias: &[f32], out_dim: usize, out: &mut [f32]) {
    for j in 0..out_dim {
        let mut sum = bias[j];
        for (i, x) in input.iter().enumerate() {
            sum += *x * weight[i * out_dim + j];
        }
        out[j] = sum;
    }
}

fn linear_backward(
    input: &[f32],
    weight: &[f32],
    dout: &[f32],
    out_dim: usize,
    grad_weight: &mut [f32],
    grad_bias: &mut [f32],
    dinput: &mut [f32],
) {
    for j in 0..out_dim {
        grad_bias[j] += dout[j];
    }
    for (i, x) in input.iter().enumerate() {
        for j in 0..out_dim {
            grad_weight[i * out_dim + j] += *x * dout[j];
            dinput[i] += weight[i * out_dim + j] * dout[j];
        }
    }
}

#[derive(Debug, Clone)]
struct TinyTransformer {
    vocab_size: usize,
    seq_len: usize,
    tok_emb: Vec<f32>,
    pos_emb: Vec<f32>,
    ln1_g: Vec<f32>,
    ln1_b: Vec<f32>,
    wq: Vec<f32>,
    bq: Vec<f32>,
    wk: Vec<f32>,
    bk: Vec<f32>,
    wv: Vec<f32>,
    bv: Vec<f32>,
    wo: Vec<f32>,
    bo: Vec<f32>,
    ln2_g: Vec<f32>,
    ln2_b: Vec<f32>,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: Vec<f32>,
    lnf_g: Vec<f32>,
    lnf_b: Vec<f32>,
    lm_w: Vec<f32>,
    lm_b: Vec<f32>,
}

#[derive(Debug, Clone)]
struct TinyTransformerGrads {
    tok_emb: Vec<f32>,
    pos_emb: Vec<f32>,
    ln1_g: Vec<f32>,
    ln1_b: Vec<f32>,
    wq: Vec<f32>,
    bq: Vec<f32>,
    wk: Vec<f32>,
    bk: Vec<f32>,
    wv: Vec<f32>,
    bv: Vec<f32>,
    wo: Vec<f32>,
    bo: Vec<f32>,
    ln2_g: Vec<f32>,
    ln2_b: Vec<f32>,
    w1: Vec<f32>,
    b1: Vec<f32>,
    w2: Vec<f32>,
    b2: Vec<f32>,
    lnf_g: Vec<f32>,
    lnf_b: Vec<f32>,
    lm_w: Vec<f32>,
    lm_b: Vec<f32>,
}

impl TinyTransformerGrads {
    fn zero_like(model: &TinyTransformer) -> Self {
        Self {
            tok_emb: vec![0.0; model.tok_emb.len()],
            pos_emb: vec![0.0; model.pos_emb.len()],
            ln1_g: vec![0.0; model.ln1_g.len()],
            ln1_b: vec![0.0; model.ln1_b.len()],
            wq: vec![0.0; model.wq.len()],
            bq: vec![0.0; model.bq.len()],
            wk: vec![0.0; model.wk.len()],
            bk: vec![0.0; model.bk.len()],
            wv: vec![0.0; model.wv.len()],
            bv: vec![0.0; model.bv.len()],
            wo: vec![0.0; model.wo.len()],
            bo: vec![0.0; model.bo.len()],
            ln2_g: vec![0.0; model.ln2_g.len()],
            ln2_b: vec![0.0; model.ln2_b.len()],
            w1: vec![0.0; model.w1.len()],
            b1: vec![0.0; model.b1.len()],
            w2: vec![0.0; model.w2.len()],
            b2: vec![0.0; model.b2.len()],
            lnf_g: vec![0.0; model.lnf_g.len()],
            lnf_b: vec![0.0; model.lnf_b.len()],
            lm_w: vec![0.0; model.lm_w.len()],
            lm_b: vec![0.0; model.lm_b.len()],
        }
    }
}

impl TinyTransformer {
    fn new(vocab_size: usize, seq_len: usize, rng: &mut SimpleRng) -> Self {
        fn init_vec(len: usize, scale: f32, rng: &mut SimpleRng) -> Vec<f32> {
            let mut v = vec![0.0; len];
            for x in &mut v {
                *x = rng.gen_f32_range(-scale, scale);
            }
            v
        }

        Self {
            vocab_size,
            seq_len,
            tok_emb: init_vec(vocab_size * MODEL_DIM, 0.05, rng),
            pos_emb: init_vec(seq_len * MODEL_DIM, 0.05, rng),
            ln1_g: vec![1.0; MODEL_DIM],
            ln1_b: vec![0.0; MODEL_DIM],
            wq: init_vec(MODEL_DIM * MODEL_DIM, 0.05, rng),
            bq: vec![0.0; MODEL_DIM],
            wk: init_vec(MODEL_DIM * MODEL_DIM, 0.05, rng),
            bk: vec![0.0; MODEL_DIM],
            wv: init_vec(MODEL_DIM * MODEL_DIM, 0.05, rng),
            bv: vec![0.0; MODEL_DIM],
            wo: init_vec(MODEL_DIM * MODEL_DIM, 0.05, rng),
            bo: vec![0.0; MODEL_DIM],
            ln2_g: vec![1.0; MODEL_DIM],
            ln2_b: vec![0.0; MODEL_DIM],
            w1: init_vec(MODEL_DIM * MLP_DIM, 0.05, rng),
            b1: vec![0.0; MLP_DIM],
            w2: init_vec(MLP_DIM * MODEL_DIM, 0.05, rng),
            b2: vec![0.0; MODEL_DIM],
            lnf_g: vec![1.0; MODEL_DIM],
            lnf_b: vec![0.0; MODEL_DIM],
            lm_w: init_vec(MODEL_DIM * vocab_size, 0.05, rng),
            lm_b: vec![0.0; vocab_size],
        }
    }

    fn train_step(
        &mut self,
        data: &[u16],
        batch_size: usize,
        seq_len: usize,
        lr: f32,
        deadline: Instant,
        rng: &mut SimpleRng,
    ) -> AppResult<(f64, usize)> {
        if data.len() <= seq_len + 1 {
            return Err(format!("dataset too short for seq_len={seq_len}").into());
        }
        if seq_len != self.seq_len {
            return Err(format!(
                "runtime seq_len {} does not match model seq_len {}",
                seq_len, self.seq_len
            )
            .into());
        }

        let mut grads = TinyTransformerGrads::zero_like(self);
        let mut total_loss = 0.0_f64;
        let mut nseq = 0usize;
        for _ in 0..batch_size {
            if Instant::now() >= deadline && nseq > 0 {
                break;
            }
            let start = rng.gen_range(data.len() - seq_len - 1);
            let seq = &data[start..start + seq_len + 1];
            total_loss += self.forward_backward_sequence(seq, &mut grads)?;
            nseq += 1;
        }

        if nseq == 0 {
            return Err("train_step processed zero sequences before deadline".into());
        }
        let ntokens = nseq * seq_len;
        let scale = lr / (ntokens as f32);
        self.apply_grads(&grads, scale);
        Ok((total_loss / ntokens as f64, ntokens))
    }

    fn evaluate_bpb(
        &self,
        data: &[u16],
        token_bytes: &[u32],
        batch_size: usize,
        seq_len: usize,
        eval_batches: usize,
        deadline: Option<Instant>,
        rng: &mut SimpleRng,
    ) -> AppResult<f64> {
        if data.len() <= seq_len + 1 {
            return Err(format!("dataset too short for eval seq_len={seq_len}").into());
        }
        if seq_len != self.seq_len {
            return Err(format!(
                "runtime seq_len {} does not match model seq_len {}",
                seq_len, self.seq_len
            )
            .into());
        }

        let mut total_nats = 0.0_f64;
        let mut total_bytes = 0_u64;
        let mut num_eval = 0usize;
        'outer: for _ in 0..eval_batches {
            for _ in 0..batch_size {
                let start = rng.gen_range(data.len() - seq_len - 1);
                let seq = &data[start..start + seq_len + 1];
                total_nats += self.sequence_nll_nats(seq)?;
                for t in 0..seq_len {
                    total_bytes += token_bytes[seq[t + 1] as usize] as u64;
                }
                num_eval += 1;
                if let Some(dl) = deadline {
                    if Instant::now() >= dl && num_eval > 0 {
                        break 'outer;
                    }
                }
            }
        }

        if total_bytes == 0 {
            return Err("total_bytes was zero during evaluation".into());
        }
        Ok(nats_to_bpb(total_nats, total_bytes))
    }

    fn sequence_nll_nats(&self, seq: &[u16]) -> AppResult<f64> {
        let tlen = seq.len() - 1;
        let mut x0 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln1_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln1_xhat = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut q = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut k = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut v = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut probs = vec![vec![0.0_f32; tlen]; tlen];
        let mut ctx = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut attn_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut x1 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln2_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln2_xhat = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut mlp_h = vec![vec![0.0_f32; MLP_DIM]; tlen];
        let mut mlp_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut x2 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut lnf_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut lnf_xhat = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut logits = vec![0.0_f32; self.vocab_size];
        let mut sm = vec![0.0_f32; self.vocab_size];
        let scale = 1.0_f32 / (MODEL_DIM as f32).sqrt();

        for t in 0..tlen {
            let tok = seq[t] as usize;
            for d in 0..MODEL_DIM {
                x0[t][d] = self.tok_emb[tok * MODEL_DIM + d] + self.pos_emb[t * MODEL_DIM + d];
            }
            layer_norm_forward(
                &x0[t],
                &self.ln1_g,
                &self.ln1_b,
                &mut ln1_out[t],
                &mut ln1_xhat[t],
            );
            linear_forward(&ln1_out[t], &self.wq, &self.bq, MODEL_DIM, &mut q[t]);
            linear_forward(&ln1_out[t], &self.wk, &self.bk, MODEL_DIM, &mut k[t]);
            linear_forward(&ln1_out[t], &self.wv, &self.bv, MODEL_DIM, &mut v[t]);
        }

        for t in 0..tlen {
            let mut attn_logits = vec![f32::NEG_INFINITY; tlen];
            for (s, slot) in attn_logits.iter_mut().enumerate().take(t + 1) {
                let dot = q[t]
                    .iter()
                    .zip(k[s].iter())
                    .map(|(a, b)| a * b)
                    .sum::<f32>();
                *slot = dot * scale;
            }
            stable_softmax(&attn_logits[..=t], &mut probs[t][..=t]);
            for s in 0..=t {
                for d in 0..MODEL_DIM {
                    ctx[t][d] += probs[t][s] * v[s][d];
                }
            }
            linear_forward(&ctx[t], &self.wo, &self.bo, MODEL_DIM, &mut attn_out[t]);
            for d in 0..MODEL_DIM {
                x1[t][d] = x0[t][d] + attn_out[t][d];
            }
            layer_norm_forward(
                &x1[t],
                &self.ln2_g,
                &self.ln2_b,
                &mut ln2_out[t],
                &mut ln2_xhat[t],
            );
            linear_forward(&ln2_out[t], &self.w1, &self.b1, MLP_DIM, &mut mlp_h[t]);
            for j in 0..MLP_DIM {
                mlp_h[t][j] = relu(mlp_h[t][j]);
            }
            linear_forward(&mlp_h[t], &self.w2, &self.b2, MODEL_DIM, &mut mlp_out[t]);
            for d in 0..MODEL_DIM {
                x2[t][d] = x1[t][d] + mlp_out[t][d];
            }
            layer_norm_forward(
                &x2[t],
                &self.lnf_g,
                &self.lnf_b,
                &mut lnf_out[t],
                &mut lnf_xhat[t],
            );
        }

        let mut total_nats = 0.0_f64;
        for t in 0..tlen {
            linear_forward(
                &lnf_out[t],
                &self.lm_w,
                &self.lm_b,
                self.vocab_size,
                &mut logits,
            );
            stable_softmax(&logits, &mut sm);
            let target = seq[t + 1] as usize;
            let p = sm[target].max(1e-12);
            total_nats += -(p as f64).ln();
        }
        Ok(total_nats)
    }

    fn forward_backward_sequence(
        &self,
        seq: &[u16],
        grads: &mut TinyTransformerGrads,
    ) -> AppResult<f64> {
        let tlen = seq.len() - 1;
        if tlen != self.seq_len {
            return Err(format!("sequence length {} != configured {}", tlen, self.seq_len).into());
        }
        let scale = 1.0_f32 / (MODEL_DIM as f32).sqrt();

        let mut x0 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln1_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln1_xhat = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln1_inv = vec![0.0_f32; tlen];
        let mut q = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut k = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut v = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut attn_scores = vec![vec![f32::NEG_INFINITY; tlen]; tlen];
        let mut attn_probs = vec![vec![0.0_f32; tlen]; tlen];
        let mut ctx = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut attn_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut x1 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln2_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln2_xhat = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut ln2_inv = vec![0.0_f32; tlen];
        let mut mlp_pre = vec![vec![0.0_f32; MLP_DIM]; tlen];
        let mut mlp_h = vec![vec![0.0_f32; MLP_DIM]; tlen];
        let mut mlp_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut x2 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut lnf_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut lnf_xhat = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut lnf_inv = vec![0.0_f32; tlen];
        let mut logits = vec![vec![0.0_f32; self.vocab_size]; tlen];
        let mut probs = vec![vec![0.0_f32; self.vocab_size]; tlen];

        for t in 0..tlen {
            let tok = seq[t] as usize;
            for d in 0..MODEL_DIM {
                x0[t][d] = self.tok_emb[tok * MODEL_DIM + d] + self.pos_emb[t * MODEL_DIM + d];
            }
            ln1_inv[t] = layer_norm_forward(
                &x0[t],
                &self.ln1_g,
                &self.ln1_b,
                &mut ln1_out[t],
                &mut ln1_xhat[t],
            );
            linear_forward(&ln1_out[t], &self.wq, &self.bq, MODEL_DIM, &mut q[t]);
            linear_forward(&ln1_out[t], &self.wk, &self.bk, MODEL_DIM, &mut k[t]);
            linear_forward(&ln1_out[t], &self.wv, &self.bv, MODEL_DIM, &mut v[t]);
        }

        for t in 0..tlen {
            for s in 0..=t {
                let dot = q[t]
                    .iter()
                    .zip(k[s].iter())
                    .map(|(a, b)| a * b)
                    .sum::<f32>();
                attn_scores[t][s] = dot * scale;
            }
            stable_softmax(&attn_scores[t][..=t], &mut attn_probs[t][..=t]);
            for s in 0..=t {
                for d in 0..MODEL_DIM {
                    ctx[t][d] += attn_probs[t][s] * v[s][d];
                }
            }

            linear_forward(&ctx[t], &self.wo, &self.bo, MODEL_DIM, &mut attn_out[t]);
            for d in 0..MODEL_DIM {
                x1[t][d] = x0[t][d] + attn_out[t][d];
            }
            ln2_inv[t] = layer_norm_forward(
                &x1[t],
                &self.ln2_g,
                &self.ln2_b,
                &mut ln2_out[t],
                &mut ln2_xhat[t],
            );
            linear_forward(&ln2_out[t], &self.w1, &self.b1, MLP_DIM, &mut mlp_pre[t]);
            for j in 0..MLP_DIM {
                mlp_h[t][j] = relu(mlp_pre[t][j]);
            }
            linear_forward(&mlp_h[t], &self.w2, &self.b2, MODEL_DIM, &mut mlp_out[t]);
            for d in 0..MODEL_DIM {
                x2[t][d] = x1[t][d] + mlp_out[t][d];
            }
            lnf_inv[t] = layer_norm_forward(
                &x2[t],
                &self.lnf_g,
                &self.lnf_b,
                &mut lnf_out[t],
                &mut lnf_xhat[t],
            );
            linear_forward(
                &lnf_out[t],
                &self.lm_w,
                &self.lm_b,
                self.vocab_size,
                &mut logits[t],
            );
            stable_softmax(&logits[t], &mut probs[t]);
        }

        let mut loss = 0.0_f64;
        let mut d_lnf_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            let target = seq[t + 1] as usize;
            let p = probs[t][target].max(1e-12);
            loss += -(p as f64).ln();

            let mut d_logits = probs[t].clone();
            d_logits[target] -= 1.0;
            for (vocab_idx, dlogit) in d_logits.iter().enumerate() {
                grads.lm_b[vocab_idx] += *dlogit;
            }
            for d in 0..MODEL_DIM {
                for (vocab_idx, dlogit) in d_logits.iter().enumerate() {
                    grads.lm_w[d * self.vocab_size + vocab_idx] += lnf_out[t][d] * *dlogit;
                    d_lnf_out[t][d] += self.lm_w[d * self.vocab_size + vocab_idx] * *dlogit;
                }
            }
        }

        let mut d_x2 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            layer_norm_backward(
                &d_lnf_out[t],
                &lnf_xhat[t],
                lnf_inv[t],
                &self.lnf_g,
                &mut grads.lnf_g,
                &mut grads.lnf_b,
                &mut d_x2[t],
            );
        }

        let mut d_x1 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut d_mlp_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            for d in 0..MODEL_DIM {
                d_x1[t][d] += d_x2[t][d];
                d_mlp_out[t][d] += d_x2[t][d];
            }
        }

        let mut d_mlp_h = vec![vec![0.0_f32; MLP_DIM]; tlen];
        for t in 0..tlen {
            linear_backward(
                &mlp_h[t],
                &self.w2,
                &d_mlp_out[t],
                MODEL_DIM,
                &mut grads.w2,
                &mut grads.b2,
                &mut d_mlp_h[t],
            );
        }

        let mut d_mlp_pre = vec![vec![0.0_f32; MLP_DIM]; tlen];
        for t in 0..tlen {
            for j in 0..MLP_DIM {
                d_mlp_pre[t][j] = if mlp_pre[t][j] > 0.0 {
                    d_mlp_h[t][j]
                } else {
                    0.0
                };
            }
        }

        let mut d_ln2_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            linear_backward(
                &ln2_out[t],
                &self.w1,
                &d_mlp_pre[t],
                MLP_DIM,
                &mut grads.w1,
                &mut grads.b1,
                &mut d_ln2_out[t],
            );
        }

        for t in 0..tlen {
            layer_norm_backward(
                &d_ln2_out[t],
                &ln2_xhat[t],
                ln2_inv[t],
                &self.ln2_g,
                &mut grads.ln2_g,
                &mut grads.ln2_b,
                &mut d_x1[t],
            );
        }

        let mut d_x0 = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut d_attn_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            for d in 0..MODEL_DIM {
                d_x0[t][d] += d_x1[t][d];
                d_attn_out[t][d] += d_x1[t][d];
            }
        }

        let mut d_ctx = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            linear_backward(
                &ctx[t],
                &self.wo,
                &d_attn_out[t],
                MODEL_DIM,
                &mut grads.wo,
                &mut grads.bo,
                &mut d_ctx[t],
            );
        }

        let mut d_probs = vec![vec![0.0_f32; tlen]; tlen];
        let mut d_v = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            for s in 0..=t {
                for d in 0..MODEL_DIM {
                    d_probs[t][s] += d_ctx[t][d] * v[s][d];
                    d_v[s][d] += d_ctx[t][d] * attn_probs[t][s];
                }
            }
        }

        let mut d_scores = vec![vec![0.0_f32; tlen]; tlen];
        for t in 0..tlen {
            let mut dot = 0.0_f32;
            for s in 0..=t {
                dot += d_probs[t][s] * attn_probs[t][s];
            }
            for s in 0..=t {
                d_scores[t][s] = attn_probs[t][s] * (d_probs[t][s] - dot);
            }
        }

        let mut d_q = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        let mut d_k = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            for s in 0..=t {
                let g = d_scores[t][s] * scale;
                for d in 0..MODEL_DIM {
                    d_q[t][d] += g * k[s][d];
                    d_k[s][d] += g * q[t][d];
                }
            }
        }

        let mut d_ln1_out = vec![vec![0.0_f32; MODEL_DIM]; tlen];
        for t in 0..tlen {
            linear_backward(
                &ln1_out[t],
                &self.wq,
                &d_q[t],
                MODEL_DIM,
                &mut grads.wq,
                &mut grads.bq,
                &mut d_ln1_out[t],
            );
            linear_backward(
                &ln1_out[t],
                &self.wk,
                &d_k[t],
                MODEL_DIM,
                &mut grads.wk,
                &mut grads.bk,
                &mut d_ln1_out[t],
            );
            linear_backward(
                &ln1_out[t],
                &self.wv,
                &d_v[t],
                MODEL_DIM,
                &mut grads.wv,
                &mut grads.bv,
                &mut d_ln1_out[t],
            );
        }

        for t in 0..tlen {
            layer_norm_backward(
                &d_ln1_out[t],
                &ln1_xhat[t],
                ln1_inv[t],
                &self.ln1_g,
                &mut grads.ln1_g,
                &mut grads.ln1_b,
                &mut d_x0[t],
            );
        }

        for t in 0..tlen {
            let tok = seq[t] as usize;
            for d in 0..MODEL_DIM {
                grads.tok_emb[tok * MODEL_DIM + d] += d_x0[t][d];
                grads.pos_emb[t * MODEL_DIM + d] += d_x0[t][d];
            }
        }

        Ok(loss)
    }

    fn apply_grads(&mut self, grads: &TinyTransformerGrads, scale: f32) {
        fn apply_param(param: &mut [f32], grad: &[f32], scale: f32) {
            for (p, g) in param.iter_mut().zip(grad.iter()) {
                *p -= scale * *g;
            }
        }
        apply_param(&mut self.tok_emb, &grads.tok_emb, scale);
        apply_param(&mut self.pos_emb, &grads.pos_emb, scale);
        apply_param(&mut self.ln1_g, &grads.ln1_g, scale);
        apply_param(&mut self.ln1_b, &grads.ln1_b, scale);
        apply_param(&mut self.wq, &grads.wq, scale);
        apply_param(&mut self.bq, &grads.bq, scale);
        apply_param(&mut self.wk, &grads.wk, scale);
        apply_param(&mut self.bk, &grads.bk, scale);
        apply_param(&mut self.wv, &grads.wv, scale);
        apply_param(&mut self.bv, &grads.bv, scale);
        apply_param(&mut self.wo, &grads.wo, scale);
        apply_param(&mut self.bo, &grads.bo, scale);
        apply_param(&mut self.ln2_g, &grads.ln2_g, scale);
        apply_param(&mut self.ln2_b, &grads.ln2_b, scale);
        apply_param(&mut self.w1, &grads.w1, scale);
        apply_param(&mut self.b1, &grads.b1, scale);
        apply_param(&mut self.w2, &grads.w2, scale);
        apply_param(&mut self.b2, &grads.b2, scale);
        apply_param(&mut self.lnf_g, &grads.lnf_g, scale);
        apply_param(&mut self.lnf_b, &grads.lnf_b, scale);
        apply_param(&mut self.lm_w, &grads.lm_w, scale);
        apply_param(&mut self.lm_b, &grads.lm_b, scale);
    }

    fn save_checkpoint(&self, path: &PathBuf) -> AppResult<()> {
        let mut buf = Vec::new();
        let header = format!(
            "tiny_transformer_v1 {} {} {} {}\n",
            self.vocab_size, self.seq_len, MODEL_DIM, MLP_DIM
        );
        buf.extend_from_slice(header.as_bytes());
        let tensors = [
            &self.tok_emb,
            &self.pos_emb,
            &self.ln1_g,
            &self.ln1_b,
            &self.wq,
            &self.bq,
            &self.wk,
            &self.bk,
            &self.wv,
            &self.bv,
            &self.wo,
            &self.bo,
            &self.ln2_g,
            &self.ln2_b,
            &self.w1,
            &self.b1,
            &self.w2,
            &self.b2,
            &self.lnf_g,
            &self.lnf_b,
            &self.lm_w,
            &self.lm_b,
        ];
        for tensor in tensors {
            for v in tensor {
                buf.extend_from_slice(&v.to_le_bytes());
            }
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
    let mut model = TinyTransformer::new(tokenizer.vocab.len(), args.seq_len, &mut rng);

    let t0_total = Instant::now();
    let t0_train = Instant::now();
    let budget = Duration::from_secs(args.time_budget_seconds);

    let mut step: u64 = 0;
    let mut total_tokens: u64 = 0;
    let mut last_val_bpb = f64::INFINITY;
    let deadline = t0_train + budget;

    while Instant::now() < deadline {
        step += 1;
        let (train_loss, step_tokens) = model.train_step(
            &train_tokens,
            args.batch_size,
            args.seq_len,
            args.learning_rate,
            deadline,
            &mut rng,
        )?;
        total_tokens += step_tokens as u64;

        if step % args.eval_interval == 0 {
            last_val_bpb = model.evaluate_bpb(
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
            Some(deadline),
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
