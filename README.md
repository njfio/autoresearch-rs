# autoresearch-rs

Rust-first autonomous research sandbox inspired by [karpathy/autoresearch](https://github.com/karpathy/autoresearch).

This project keeps the same workflow spirit:
- one-time data preparation,
- fixed wall-clock experiment runs (default 5 minutes),
- autonomous agent loop instructions,
- run logging plus best-run tracking.

## Quickstart

Requirements:
- Rust stable toolchain (`rustup`, `cargo`)
- CPU is enough (default path)
- Internet only for `prepare` corpus download
- `curl` available on PATH

```bash
# 1) Prepare artifacts (download tinyshakespeare + build char tokenizer)
cargo run --release --bin prepare

# 2) Run one fixed-time experiment (default 300s)
cargo run --release --bin train

# 3) Inspect best/latest run metadata
cargo run --release --bin report
```

Artifacts are stored locally in:
- `artifacts/` (dataset + tokenizer)
- `runs/` (per-run checkpoints, config, metrics, summaries, results table)

## Command surface

- `cargo run --bin prepare`
  - Downloads Tiny Shakespeare-like corpus.
  - Builds char-level tokenizer/vocab.
  - Saves `tokenizer.txt`, `train_tokens.bin`, `val_tokens.bin`, and prep metadata.

- `cargo run --bin train`
  - Trains a tiny GPT (Candle tensor core) with token + position embeddings, multi-layer masked multi-head self-attention, MLP, residuals, layer norms, and LM head for a fixed wall-clock budget.
  - Optimizer is AdamW with warmup + cosine LR schedule.
  - Tracks validation `val_bpb` (bits per byte), analogous to Python repo metric.
  - Writes checkpoint + metadata in `runs/<run_id>/`.
  - Appends summary row to `runs/results.tsv` and updates `runs/best.txt` when improved.
  - Tunable knobs:
    - model shape: `--n-layers`, `--n-heads`, `--d-model`, `--d-ff`, `--dropout`
    - optimization: `--learning-rate`, `--lr-warmup-steps`, `--lr-min-scale` (`--lr-final-scale` alias), `--weight-decay`, `--grad-clip-norm`
    - runtime: `--batch-size`, `--seq-len`, `--time-budget-seconds`, `--eval-interval`, `--eval-batches`
  - Compatibility aliases preserved: `--model-dim -> --d-model`, `--mlp-dim -> --d-ff`.

- `cargo run --bin report`
  - Prints `latest` and `best` run summaries.

- `cargo run --bin autoresearch -- --experiments 20`
  - Runs an autonomous experiment loop.
  - Mutates a small set of hyperparameters each run (`batch_size`, `seq_len`, `learning_rate`, `lr_warmup_steps`, `lr_min_scale`, `grad_clip_norm`, `n_layers`, `n_heads`, `d_model`, `d_ff`, `dropout`, `weight_decay`) and executes fixed-time `train` runs.
  - Logs each run to `runs/results.tsv` and surfaces keep/discard decisions.

## Metric definition

This repo reports **validation bits per byte (val_bpb)**:

`val_bpb = total_nats / (ln(2) * total_target_bytes)`

- `total_nats`: summed negative log-likelihood over validation targets.
- `total_target_bytes`: summed UTF-8 byte lengths for each target token.
- Computation stays byte-exact against tokenizer artifacts (`tokenizer.txt`) so results remain stable across runs and model upgrades.

Lower is better.

## Parity mapping to Python autoresearch

- Python `prepare.py` -> Rust `src/bin/prepare.rs`
  - Same role: data acquisition + tokenizer artifact generation.

- Python `train.py` -> Rust `src/bin/train.rs`
  - Same role: fixed-time training run and val_bpb evaluation/logging.
  - Current Rust parity status:
    - done: tensor-backed training core (Candle), tiny GPT architecture, AdamW, warmup+cosine schedule, fixed-time budget, val_bpb logging contracts
    - still missing for full parity: broader model scale/runtime tuning and generation-side quality-of-life features from upstream Python ecosystem

- Python `program.md` -> Rust `program.md`
  - Same role: instructions for autonomous iterative experimentation.

- Python `results.tsv` pattern -> Rust `runs/results.tsv`
  - Keep/discard best-run workflow preserved.

## Practical defaults and constraints

- CPU-first by default; intentionally lightweight and self-contained.
- No CUDA dependency is required.
- Optional GPU support is not implemented in code yet. Suggested route:
  - swap model core to a GPU-aware crate,
  - keep the same CLI and run logging contracts.

Recommended 5-minute starter config:

```bash
cargo run --release --bin train -- \
  --time-budget-seconds 300 \
  --batch-size 32 \
  --seq-len 64 \
  --n-layers 2 \
  --n-heads 4 \
  --d-model 64 \
  --d-ff 256 \
  --dropout 0.0 \
  --learning-rate 0.003 \
  --lr-warmup-steps 200 \
  --lr-min-scale 0.2 \
  --weight-decay 0.1 \
  --grad-clip-norm 1.0
```

## Suggested autonomous loop

Use `program.md` as the agent instruction baseline. The intended iteration loop is:
1. edit training code,
2. run a 5-minute experiment,
3. compare `val_bpb`,
4. keep/discard and continue.

## Notes

- The baseline is intentionally tiny and practical on CPU while remaining a true Transformer LM.
