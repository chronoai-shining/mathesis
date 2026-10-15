//! Export the actual contract-3 referee prompt for one statement-only case.
//! No provider call, ground-truth label or publication action occurs here.
use mathesis_review::{Document, Statement, referee_prompts};
use serde::Deserialize;
use std::io::{self, Read};

#[derive(Deserialize)]
struct Case {
    id: String,
    statement: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().take(100_001).read_to_string(&mut input)?;
    if input.len() > 100_000 {
        return Err("evaluation input exceeds limit".into());
    }
    let case: Case = serde_json::from_str(&input)?;
    let document = Document {
        title: format!("Statement-only evaluation {}", case.id),
        abstract_text: "Determine the known/open status of the supplied mathematical assertion. No proof is submitted.".into(),
        text: Some(case.statement.clone()),
    };
    let statement = Statement {
        id: "C1".into(),
        label: "Question".into(),
        statement: case.statement,
        main: true,
        proved: false,
        depends_on: vec![],
    };
    let prompt = referee_prompts::methodology_referee(
        &document,
        std::slice::from_ref(&statement),
        &serde_json::json!([{ "id": "C1", "settles": null }]),
        true,
    );
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "model": "oracle/chrono-chatgpt-pro-pool",
            "stream": true,
            "messages": [{ "role": "user", "content": prompt }]
        }))?
    );
    Ok(())
}
