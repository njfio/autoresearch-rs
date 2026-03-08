use std::env;
use std::path::PathBuf;

use autoresearch_rs::{
    download_to_path, ensure_dir, read_text, split_train_val, write_kv, write_tokenizer,
    write_u16_tokens, AppResult, CharTokenizer, DEFAULT_CORPUS_URL,
};

#[derive(Debug, Clone)]
struct Args {
    artifacts_dir: PathBuf,
    corpus_url: String,
    val_fraction: f32,
    force_download: bool,
}

fn parse_args() -> AppResult<Args> {
    let mut artifacts_dir = PathBuf::from("artifacts");
    let mut corpus_url = DEFAULT_CORPUS_URL.to_string();
    let mut val_fraction = 0.1_f32;
    let mut force_download = false;

    let mut it = env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--artifacts-dir" => {
                let v = it.next().ok_or("missing value for --artifacts-dir")?;
                artifacts_dir = PathBuf::from(v);
            }
            "--corpus-url" => {
                corpus_url = it.next().ok_or("missing value for --corpus-url")?;
            }
            "--val-fraction" => {
                val_fraction = it
                    .next()
                    .ok_or("missing value for --val-fraction")?
                    .parse()?;
            }
            "--force-download" => force_download = true,
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    Ok(Args {
        artifacts_dir,
        corpus_url,
        val_fraction,
        force_download,
    })
}

fn print_help() {
    println!("prepare usage:");
    println!("  cargo run --bin prepare -- [options]");
    println!("options:");
    println!("  --artifacts-dir <path>   default: artifacts");
    println!("  --corpus-url <url>       default: tinyshakespeare URL");
    println!("  --val-fraction <float>   default: 0.1");
    println!("  --force-download");
}

fn main() -> AppResult<()> {
    let args = parse_args()?;
    ensure_dir(&args.artifacts_dir)?;

    let corpus_path = args.artifacts_dir.join("tinyshakespeare.txt");
    if args.force_download || !corpus_path.exists() {
        println!("downloading corpus from {}", args.corpus_url);
        download_to_path(&args.corpus_url, &corpus_path)?;
    } else {
        println!("corpus exists at {} (skip download)", corpus_path.display());
    }

    let text = read_text(&corpus_path)?;
    let (train_text, val_text) = split_train_val(&text, args.val_fraction)?;

    let tokenizer = CharTokenizer::from_text(&text)?;
    let train_tokens = tokenizer.encode(train_text)?;
    let val_tokens = tokenizer.encode(val_text)?;

    let tokenizer_path = args.artifacts_dir.join("tokenizer.txt");
    let train_path = args.artifacts_dir.join("train_tokens.bin");
    let val_path = args.artifacts_dir.join("val_tokens.bin");
    let metadata_path = args.artifacts_dir.join("prepare_metadata.txt");

    write_tokenizer(&tokenizer_path, &tokenizer.to_artifacts())?;
    write_u16_tokens(&train_path, &train_tokens)?;
    write_u16_tokens(&val_path, &val_tokens)?;

    write_kv(
        &metadata_path,
        &[
            ("corpus_path", corpus_path.display().to_string()),
            ("tokenizer_path", tokenizer_path.display().to_string()),
            ("train_tokens_path", train_path.display().to_string()),
            ("val_tokens_path", val_path.display().to_string()),
            ("train_tokens", train_tokens.len().to_string()),
            ("val_tokens", val_tokens.len().to_string()),
            ("vocab_size", tokenizer.vocab_size().to_string()),
            ("val_fraction", args.val_fraction.to_string()),
        ],
    )?;

    println!("prepared artifacts in {}", args.artifacts_dir.display());
    println!("vocab_size: {}", tokenizer.vocab_size());
    println!("train_tokens: {}", train_tokens.len());
    println!("val_tokens: {}", val_tokens.len());
    Ok(())
}
