//! Exercise the production reader on downloaded immutable artifacts.
//! Arguments: projection-dir original-report-dir admitted-identity.json [module paths...]
use mathesis_review::knowledge::{self, PublicationIdentity};
use std::{collections::BTreeMap, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let projection = PathBuf::from(args.next().ok_or("missing projection directory")?);
    let report = PathBuf::from(args.next().ok_or("missing original report directory")?);
    let identity: PublicationIdentity =
        serde_json::from_slice(&fs::read(args.next().ok_or("missing admitted identity")?)?)?;
    let modules = args.collect();
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
    let mut sidecars = BTreeMap::new();
    for name in [
        "raw-lean-report.json.sha256",
        "raw-lean-report.json.input.attestation",
        "raw-lean-report.json.provenance.json",
    ] {
        sidecars.insert(name.to_owned(), fs::read(report.join(name))?);
    }
    let archive = fs::File::open(report.join("raw-lean-report.json.materials.zip"))?;
    let publication = knowledge::consume(&identity, &files, &sidecars, archive, &modules)?;
    serde_json::to_writer(std::io::stdout().lock(), &publication)?;
    Ok(())
}
