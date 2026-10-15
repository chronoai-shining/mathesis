//! Replay one finite arithmetic input from stdin; no model, network or Lean.
use std::io::{self, Read};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = vec![];
    io::stdin()
        .take(mathesis_review::computation::MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    println!(
        "{}",
        serde_json::to_string(&mathesis_review::computation::replay(&bytes))?
    );
    Ok(())
}
