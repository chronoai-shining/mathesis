//! Binary composition of read-only publication ingestion and durable storage.
//! Operator admission is explicit; artifact self-assertions never select trust.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use mathesis_core::{CoreError, CoreResult, ports::BlobStore};
use mathesis_review::knowledge::{
    self, PublicationIdentity,
    index::{SearchIndex, SourceIndex},
};
use serde::Deserialize;

use crate::store::{MongoStore, knowledge::KnowledgeSnapshot};

const PROJECTION_FILES: &[&str] = &[
    "SHA256SUMS",
    "release-manifest.v1.json",
    "truth-release-publication.v1.json",
    "raw-lean-report.json",
    "report-provenance.v1.json",
    "source-snapshot.v1.json",
    "truth-export.v1.json",
    "truth-graph.v1.json",
];
const SIDECARS: &[&str] = &[
    "raw-lean-report.json.sha256",
    "raw-lean-report.json.input.attestation",
    "raw-lean-report.json.provenance.json",
];
const ARCHIVE: &str = "raw-lean-report.json.materials.zip";

#[derive(Deserialize)]
struct Admission {
    identity: PublicationIdentity,
    admitted_by: String,
    source_ci_url: String,
    report_ci_url: String,
    protected_source_evidence: String,
    license_sha256: String,
}

fn read_bounded(path: &Path, maximum: usize) -> anyhow::Result<Vec<u8>> {
    use std::io::Read;
    let metadata = std::fs::symlink_metadata(path)?;
    anyhow::ensure!(
        metadata.file_type().is_file() && metadata.len() <= maximum as u64,
        "artifact is not a bounded regular file"
    );
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= maximum,
        "artifact exceeded limit while reading"
    );
    Ok(bytes)
}

/// Imports only already-downloaded public inputs. No fetching, upstream code
/// execution, model call or source extraction occurs as a side effect.
pub(crate) async fn import(
    store: &MongoStore,
    projection: PathBuf,
    report: PathBuf,
    sources: PathBuf,
    admission_path: PathBuf,
) -> anyhow::Result<KnowledgeSnapshot> {
    let archive_path = report.join(ARCHIVE);
    let (index, files, sidecars, archive_hash, license, admission) =
        tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let admission: serde_json::Value =
                serde_json::from_slice(&read_bounded(&admission_path, 64 * 1024)?)?;
            let parsed: Admission = serde_json::from_value(admission.clone())?;
            anyhow::ensure!(
                !parsed.admitted_by.trim().is_empty()
                    && !parsed.protected_source_evidence.trim().is_empty(),
                "independent operator admission evidence is required"
            );
            for url in [&parsed.source_ci_url, &parsed.report_ci_url] {
                anyhow::ensure!(
                    url.strip_prefix(
                        "https://github.com/the-omega-institute/trureturing/actions/runs/"
                    )
                    .is_some_and(|id| !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit())),
                    "admission must retain exact source and producer CI references"
                );
            }
            let license = read_bounded(&sources.join("LICENSE"), 128 * 1024)?;
            anyhow::ensure!(
                knowledge::sha256(&license) == parsed.license_sha256,
                "source license does not match operator admission"
            );
            let files = PROJECTION_FILES
                .iter()
                .map(|name| {
                    Ok((
                        name.to_string(),
                        read_bounded(&projection.join(name), 512 * 1024 * 1024)?,
                    ))
                })
                .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
            let sidecars = SIDECARS
                .iter()
                .map(|name| {
                    Ok((
                        name.to_string(),
                        read_bounded(&report.join(name), 32 * 1024 * 1024)?,
                    ))
                })
                .collect::<anyhow::Result<BTreeMap<_, _>>>()?;
            let catalog = knowledge::catalog(&parsed.identity, &files, &BTreeSet::new())?;
            let (_, archive_hash) = knowledge::inspect_archive(
                &parsed.identity,
                &files,
                &sidecars,
                std::fs::File::open(report.join(ARCHIVE))?,
            )?;
            let root = sources.canonicalize()?;
            let index = SourceIndex::build(parsed.identity, catalog, |path| {
                let full = root.join(path);
                let real = full.canonicalize().map_err(|e| e.to_string())?;
                if !real.starts_with(&root) {
                    return Err("source outside admitted directory".into());
                }
                if !std::fs::metadata(&real)
                    .map_err(|e| e.to_string())?
                    .is_file()
                {
                    return Err("source is not a regular file".into());
                }
                // One extra byte lets the index record a size omission accurately.
                use std::io::Read;
                let mut bytes = Vec::new();
                std::fs::File::open(real)
                    .and_then(|f| {
                        f.take(knowledge::index::MAX_SOURCE_BYTES as u64 + 1)
                            .read_to_end(&mut bytes)
                    })
                    .map_err(|e| e.to_string())?;
                Ok(bytes)
            })?;
            Ok((index, files, sidecars, archive_hash, license, admission))
        })
        .await??;
    if let Some(prior) = store.knowledge_snapshot(&index.revision).await? {
        store.load_knowledge_index(&index.revision).await?;
        let expected = files
            .iter()
            .chain(sidecars.iter())
            .map(|(name, bytes)| (name.as_str(), knowledge::sha256(bytes)))
            .chain([
                (ARCHIVE, archive_hash.clone()),
                ("source-LICENSE", knowledge::sha256(&license)),
            ])
            .collect::<BTreeMap<_, _>>();
        anyhow::ensure!(
            prior.artifacts.len() == expected.len()
                && expected.iter().all(|(name, hash)| prior
                    .artifacts
                    .get(*name)
                    .is_some_and(|b| format!("sha256:{}", b.sha256) == *hash)),
            "stored snapshot artifact inventory differs from admitted inputs"
        );
        for artifact in prior.artifacts.values() {
            store.verify_knowledge_blob(artifact).await?;
        }
        return Ok(prior);
    }
    let mut artifacts = BTreeMap::new();
    artifacts.insert(
        "source-LICENSE".into(),
        store.put(&license, "text/plain").await?,
    );
    for (name, bytes) in files.into_iter().chain(sidecars) {
        artifacts.insert(name, store.put(&bytes, "application/octet-stream").await?);
    }
    // Reopen and hash actual transferred bytes to close validation/upload races.
    artifacts.insert(
        ARCHIVE.into(),
        store
            .put_knowledge_file(&archive_path, &archive_hash, 4 * 1024 * 1024 * 1024)
            .await?,
    );
    let blob = store.put_knowledge_index(&index).await?;
    let snapshot = KnowledgeSnapshot {
        id: index.revision,
        publication: index.publication,
        index: blob,
        artifacts,
        candidate_modules: index.candidate_module_count,
        indexed_modules: index.modules.len(),
        omitted_modules: index.omissions.len(),
        admitted_at: chrono::Utc::now(),
        admission,
    };
    Ok(store.insert_knowledge_snapshot(&snapshot).await?)
}

/// One cached immutable index; original Mongo/GridFS objects remain authoritative.
/// Old rounds load their own pinned revision even after configuration advances.
pub(crate) struct Knowledge {
    pub configured_revision: String,
    store: MongoStore,
    cache: tokio::sync::Mutex<Option<Arc<SearchIndex>>>,
}

impl Knowledge {
    pub(crate) fn new(store: MongoStore, configured_revision: String) -> Self {
        Self {
            configured_revision,
            store,
            cache: tokio::sync::Mutex::new(None),
        }
    }

    pub(crate) async fn index(&self, revision: &str) -> CoreResult<Arc<SearchIndex>> {
        let mut cache = self.cache.lock().await;
        if let Some(index) = cache.as_ref().filter(|i| i.identity().1 == revision) {
            return Ok(index.clone());
        }
        let source = self.store.load_knowledge_index(revision).await?;
        let index = tokio::task::spawn_blocking(move || SearchIndex::new(source))
            .await
            .map_err(|_| CoreError::Unavailable("knowledge index worker failed".into()))?
            .map_err(|e| CoreError::Unavailable(e.to_string()))?;
        let index = Arc::new(index);
        *cache = Some(index.clone());
        Ok(index)
    }
}
