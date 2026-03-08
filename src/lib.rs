use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

pub const DEFAULT_CORPUS_URL: &str =
    "https://raw.githubusercontent.com/karpathy/char-rnn/master/data/tinyshakespeare/input.txt";
pub const DEFAULT_TIME_BUDGET_SECONDS: u64 = 300;
pub const DEFAULT_SEQ_LEN: usize = 64;
pub const DEFAULT_BATCH_SIZE: usize = 32;

#[derive(Debug, Clone)]
pub struct TokenizerArtifacts {
    pub vocab: Vec<char>,
    pub token_bytes: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct CharTokenizer {
    stoi: HashMap<char, u16>,
    itos: Vec<char>,
    token_bytes: Vec<u32>,
}

impl CharTokenizer {
    pub fn from_text(text: &str) -> AppResult<Self> {
        let mut chars: Vec<char> = text.chars().collect();
        chars.sort_unstable();
        chars.dedup();

        if chars.is_empty() {
            return Err("empty corpus; cannot build tokenizer".into());
        }
        if chars.len() > u16::MAX as usize {
            return Err(format!("vocab too large for u16 token storage: {}", chars.len()).into());
        }

        let mut stoi = HashMap::with_capacity(chars.len());
        let mut token_bytes = Vec::with_capacity(chars.len());
        for (idx, ch) in chars.iter().copied().enumerate() {
            stoi.insert(ch, idx as u16);
            token_bytes.push(ch.len_utf8() as u32);
        }

        Ok(Self {
            stoi,
            itos: chars,
            token_bytes,
        })
    }

    pub fn encode(&self, text: &str) -> AppResult<Vec<u16>> {
        let mut out = Vec::with_capacity(text.chars().count());
        for ch in text.chars() {
            let id = self
                .stoi
                .get(&ch)
                .copied()
                .ok_or_else(|| format!("char not in vocab: {ch:?}"))?;
            out.push(id);
        }
        Ok(out)
    }

    pub fn vocab_size(&self) -> usize {
        self.itos.len()
    }

    pub fn to_artifacts(&self) -> TokenizerArtifacts {
        TokenizerArtifacts {
            vocab: self.itos.clone(),
            token_bytes: self.token_bytes.clone(),
        }
    }
}

pub fn ensure_dir(path: &Path) -> AppResult<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

pub fn download_to_path(url: &str, output_path: &Path) -> AppResult<()> {
    let status = Command::new("curl")
        .arg("-fL")
        .arg(url)
        .arg("-o")
        .arg(output_path)
        .status()?;
    if !status.success() {
        return Err(format!("curl failed downloading {url}").into());
    }
    Ok(())
}

pub fn read_text(path: &Path) -> AppResult<String> {
    Ok(fs::read_to_string(path)?)
}

pub fn write_u16_tokens(path: &Path, tokens: &[u16]) -> AppResult<()> {
    let mut buf = Vec::with_capacity(tokens.len() * 2);
    for t in tokens {
        buf.extend_from_slice(&t.to_le_bytes());
    }
    fs::write(path, buf)?;
    Ok(())
}

pub fn read_u16_tokens(path: &Path) -> AppResult<Vec<u16>> {
    let mut f = File::open(path)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    if buf.len() % 2 != 0 {
        return Err(format!("token file has odd byte length: {}", path.display()).into());
    }
    let mut out = Vec::with_capacity(buf.len() / 2);
    for chunk in buf.chunks_exact(2) {
        out.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok(out)
}

pub fn split_train_val(text: &str, val_fraction: f32) -> AppResult<(&str, &str)> {
    if !(0.0..1.0).contains(&val_fraction) {
        return Err(format!("val_fraction must be in [0,1), got {}", val_fraction).into());
    }
    let total_chars = text.chars().count();
    if total_chars < 100 {
        return Err(format!("corpus too small: {total_chars} chars").into());
    }
    let val_chars = ((total_chars as f32) * val_fraction).round() as usize;
    let train_chars = total_chars.saturating_sub(val_chars);
    if train_chars < 2 || val_chars < 2 {
        return Err(format!("invalid split sizes train={train_chars} val={val_chars}").into());
    }

    let split_byte = text
        .char_indices()
        .nth(train_chars)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    Ok(text.split_at(split_byte))
}

pub fn nats_to_bpb(total_nats: f64, total_bytes: u64) -> f64 {
    total_nats / (std::f64::consts::LN_2 * total_bytes as f64)
}

pub fn timestamp_run_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("run-{now}")
}

pub fn init_results_tsv(runs_dir: &Path) -> AppResult<PathBuf> {
    ensure_dir(runs_dir)?;
    let path = runs_dir.join("results.tsv");
    if !path.exists() {
        let header = "run_id\tval_bpb\ttraining_seconds\tstatus\tdescription\n";
        fs::write(&path, header)?;
    }
    Ok(path)
}

pub fn append_results_row(
    results_path: &Path,
    run_id: &str,
    val_bpb: f64,
    training_seconds: f64,
    status: &str,
    description: &str,
) -> AppResult<()> {
    let mut file = OpenOptions::new().append(true).open(results_path)?;
    writeln!(
        file,
        "{run_id}\t{val_bpb:.6}\t{training_seconds:.1}\t{status}\t{description}"
    )?;
    Ok(())
}

pub fn current_best_bpb(results_path: &Path) -> AppResult<Option<f64>> {
    if !results_path.exists() {
        return Ok(None);
    }
    let file = File::open(results_path)?;
    let reader = BufReader::new(file);
    let mut best: Option<f64> = None;
    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if i == 0 || line.trim().is_empty() {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 2 {
            continue;
        }
        if let Ok(v) = cols[1].parse::<f64>() {
            best = Some(best.map_or(v, |b| b.min(v)));
        }
    }
    Ok(best)
}

pub fn write_tokenizer(path: &Path, artifacts: &TokenizerArtifacts) -> AppResult<()> {
    let mut out = String::new();
    out.push_str(&format!("vocab_size={}\n", artifacts.vocab.len()));
    for (i, ch) in artifacts.vocab.iter().enumerate() {
        out.push_str(&format!(
            "{}\t{}\t{}\n",
            i, *ch as u32, artifacts.token_bytes[i]
        ));
    }
    fs::write(path, out)?;
    Ok(())
}

pub fn read_tokenizer(path: &Path) -> AppResult<TokenizerArtifacts> {
    let content = fs::read_to_string(path)?;
    let mut lines = content.lines();
    let header = lines
        .next()
        .ok_or_else(|| "tokenizer file missing header".to_string())?;
    let vocab_size: usize = header
        .strip_prefix("vocab_size=")
        .ok_or_else(|| "tokenizer header malformed".to_string())?
        .parse()?;

    let mut vocab = Vec::with_capacity(vocab_size);
    let mut token_bytes = Vec::with_capacity(vocab_size);

    for line in lines {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() != 3 {
            continue;
        }
        let codepoint: u32 = cols[1].parse()?;
        let ch = char::from_u32(codepoint).ok_or_else(|| "invalid char codepoint".to_string())?;
        let nbytes: u32 = cols[2].parse()?;
        vocab.push(ch);
        token_bytes.push(nbytes);
    }

    if vocab.len() != vocab_size {
        return Err(format!("tokenizer size mismatch {} != {}", vocab.len(), vocab_size).into());
    }

    Ok(TokenizerArtifacts { vocab, token_bytes })
}

pub fn write_kv(path: &Path, pairs: &[(&str, String)]) -> AppResult<()> {
    let mut out = String::new();
    for (k, v) in pairs {
        out.push_str(k);
        out.push('=');
        out.push_str(v);
        out.push('\n');
    }
    fs::write(path, out)?;
    Ok(())
}

pub fn read_kv(path: &Path) -> AppResult<HashMap<String, String>> {
    let content = fs::read_to_string(path)?;
    let mut map = HashMap::new();
    for line in content.lines() {
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
    }
    Ok(map)
}

#[derive(Debug, Clone)]
pub struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    pub fn seed(seed: u64) -> Self {
        let start = if seed == 0 { 0x9E3779B97F4A7C15 } else { seed };
        Self { state: start }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    pub fn gen_range(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }
        (self.next_u64() as usize) % upper
    }

    pub fn gen_f32_range(&mut self, low: f32, high: f32) -> f32 {
        let u = (self.next_u64() >> 40) as u32;
        let unit = (u as f32) / ((1u32 << 24) as f32);
        low + (high - low) * unit
    }
}
