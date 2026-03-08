# autoresearch-rs program

This project runs autonomous Rust-first language-model experiments.

## Setup checklist

1. Pick a run tag (e.g. `mar8-rs`) and create branch `autoresearch/<tag>`.
2. Read these files:
   - `README.md`
   - `src/bin/prepare.rs`
   - `src/bin/train.rs`
3. Ensure artifacts exist:
   - `artifacts/tokenizer.txt`
   - `artifacts/train_tokens.bin`
   - `artifacts/val_tokens.bin`
4. If missing, run:
   - `cargo run --release --bin prepare`
5. Ensure `runs/results.tsv` exists (created automatically by `train`).

## Rules

- You may edit training implementation to improve validation performance.
- Keep the run budget fixed at 5 minutes unless explicitly changed.
- Preserve run logging and summary outputs.
- Do not remove `val_bpb` reporting.

## Experiment command

```bash
cargo run --release --bin train > run.log 2>&1
```

Extract headline metrics:

```bash
grep "^val_bpb:\|^status:" run.log
```

## Expected summary block

```text
---
run_id:            run-<unix_ts>
val_bpb:           <float>
training_seconds:  ~300.0
total_seconds:     <float>
num_steps:         <int>
total_tokens:      <int>
vocab_size:        <int>
status:            keep|discard
```

## Logging and tracking

Each run writes:
- `runs/<run_id>/config.txt`
- `runs/<run_id>/metrics.tsv`
- `runs/<run_id>/checkpoint.bin`
- `runs/<run_id>/summary.txt`

Global tracking files:
- `runs/results.tsv` (append-only run table)
- `runs/latest.txt`
- `runs/best.txt` (updated on improvement)

## Autonomous loop

Repeat indefinitely until interrupted:
1. Propose one focused training change.
2. Commit the change.
3. Run one fixed-budget experiment.
4. If `val_bpb` improves, keep and continue.
5. If worse/equal, mark as discard and revert/change direction.
6. Prefer simple changes with measurable gains.

## Guidance priorities

1. Lower `val_bpb`.
2. Maintain code clarity and stability.
3. Preserve fixed-budget comparability between runs.
