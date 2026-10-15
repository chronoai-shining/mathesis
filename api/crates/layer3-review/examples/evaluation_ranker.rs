//! Measure alternative source tokenization without changing runtime defaults.
use mathesis_review::knowledge::index::{SearchIndex, SourceIndex};
use serde::Deserialize;
use std::{
    fs::File,
    io::{self, BufReader, Read},
    time::Instant,
};

#[derive(Deserialize)]
struct Query {
    id: String,
    query: String,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 2
        || !["legacy", "identifier_components", "identifier_fields"]
            .contains(&arguments[1].as_str())
    {
        return Err(
            "usage: evaluation_ranker INDEX.json legacy|identifier_components|identifier_fields < queries.json"
                .into(),
        );
    }
    let mut input = String::new();
    io::stdin().take(2_000_001).read_to_string(&mut input)?;
    if input.len() > 2_000_000 {
        return Err("too many input bytes".into());
    }
    let queries: Vec<Query> = serde_json::from_str(&input)?;
    if queries.len() > 64
        || queries
            .iter()
            .any(|q| q.id.len() > 200 || q.query.len() > 30_000)
    {
        return Err("query batch exceeds bounds".into());
    }
    let started = Instant::now();
    let source: SourceIndex = serde_json::from_reader(
        BufReader::new(File::open(&arguments[0])?).take(512 * 1024 * 1024),
    )?;
    let index = if arguments[1] == "legacy" {
        SearchIndex::new(source)?
    } else if arguments[1] == "identifier_components" {
        SearchIndex::with_identifier_components(source)?
    } else {
        SearchIndex::with_identifier_fields(source)?
    };
    let load_ms = started.elapsed().as_millis();
    let mut results = Vec::new();
    for query in queries {
        let query_start = Instant::now();
        let result = index.search(&query.query, None, 10)?;
        results.push(serde_json::json!({"id":query.id,"query":query.query,"result":result,"query_ms":query_start.elapsed().as_millis()}));
    }
    println!(
        "{}",
        serde_json::json!({"diagnostic_only":true,"algorithm":arguments[1],"load_ms":load_ms,"results":results})
    );
    Ok(())
}
