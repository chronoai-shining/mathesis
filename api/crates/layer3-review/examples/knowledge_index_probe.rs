//! Build and query a complete checked source catalog, without running Lean.
//! Arguments: projection-dir admitted-identity.json source-dir index-output query
use mathesis_review::knowledge::{
    self, PublicationIdentity,
    index::{SearchIndex, SourceIndex},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufWriter, Write},
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let projection = PathBuf::from(args.next().ok_or("missing projection directory")?);
    let identity: PublicationIdentity =
        serde_json::from_slice(&fs::read(args.next().ok_or("missing admitted identity")?)?)?;
    let sources = PathBuf::from(args.next().ok_or("missing source directory")?);
    let output = args.next().ok_or("missing index output")?;
    let query = args.next().ok_or("missing query")?;
    let mut files = BTreeMap::new();
    for name in [
        "SHA256SUMS",
        "release-manifest.v1.json",
        "truth-release-publication.v1.json",
        "raw-lean-report.json",
        "report-provenance.v1.json",
        "source-snapshot.v1.json",
        "truth-export.v1.json",
        "truth-graph.v1.json",
    ] {
        files.insert(name.to_owned(), fs::read(projection.join(name))?);
    }
    let catalog = knowledge::catalog(&identity, &files, &BTreeSet::new())?;
    let declaration_count = catalog.len();
    let index = SourceIndex::build(identity, catalog, |path| {
        fs::read(sources.join(path)).map_err(|e| e.to_string())
    })?;
    let mut writer = BufWriter::new(
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?,
    );
    serde_json::to_writer(&mut writer, &index)?;
    writer.flush()?;
    eprintln!(
        "declarations={declaration_count} candidate_modules={} indexed={} omissions={} revision={}",
        index.candidate_module_count,
        index.modules.len(),
        index.omissions.len(),
        index.revision
    );
    let result = SearchIndex::new(index)?.search(&query, None, 10)?;
    serde_json::to_writer(std::io::stdout().lock(), &result)?;
    Ok(())
}
