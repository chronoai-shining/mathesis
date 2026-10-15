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

#[derive(Debug, Clone)]
pub(crate) struct IndexPin {
    pub publication: PublicationIdentity,
    pub revision: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceRanking {
    Legacy,
    IdentifierPaths,
}
impl SourceRanking {
    pub(crate) fn protocol(self) -> Option<&'static str> {
        match self {
            Self::Legacy => None,
            Self::IdentifierPaths => Some(knowledge::index::IDENTIFIER_PATH_RANKER),
        }
    }
}

#[async_trait::async_trait]
pub(crate) trait KnowledgeSource: Send + Sync {
    async fn pin(&self) -> CoreResult<IndexPin>;
    async fn query(
        &self,
        revision: &str,
        query: &str,
        ranking: SourceRanking,
    ) -> CoreResult<knowledge::index::SourceQueryResult>;
}

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
    cache: tokio::sync::Mutex<IndexCache>,
}

#[derive(Default)]
struct IndexCache {
    loaded: Option<Arc<SearchIndex>>,
    /// Kept in the cache, never moved into a caller's future. Cancelling a
    /// review while indexing must not start another concurrent full rebuild.
    loading: Option<tokio::task::JoinHandle<CoreResult<Arc<SearchIndex>>>>,
}

impl Knowledge {
    pub(crate) fn new(store: MongoStore, configured_revision: String) -> Self {
        Self {
            configured_revision,
            store,
            cache: tokio::sync::Mutex::new(IndexCache::default()),
        }
    }

    pub(crate) async fn index(&self, revision: &str) -> CoreResult<Arc<SearchIndex>> {
        self.ranked_index(revision, SourceRanking::Legacy).await
    }

    async fn ranked_index(
        &self,
        revision: &str,
        ranking: SourceRanking,
    ) -> CoreResult<Arc<SearchIndex>> {
        let mut cache = self.cache.lock().await;
        loop {
            if let Some(index) = cache.loaded.as_ref().filter(|i| {
                i.identity().1 == revision && i.ranking_protocol() == ranking.protocol()
            }) {
                return Ok(index.clone());
            }
            if cache.loading.is_none() {
                let store = self.store.clone();
                let revision = revision.to_owned();
                cache.loading = Some(tokio::spawn(async move {
                    let source = store.load_knowledge_index(&revision).await?;
                    let index = tokio::task::spawn_blocking(move || match ranking {
                        SourceRanking::Legacy => SearchIndex::new(source),
                        SourceRanking::IdentifierPaths => {
                            SearchIndex::with_identifier_fields(source)
                        }
                    })
                    .await
                    .map_err(|_| CoreError::Unavailable("knowledge index worker failed".into()))?
                    .map_err(|e| CoreError::Unavailable(e.to_string()))?;
                    Ok(Arc::new(index))
                }));
            }
            // Await the handle by mutable reference. Dropping this future leaves
            // the original job handle in the cache for the next caller to join.
            let result = cache.loading.as_mut().expect("index load admitted").await;
            cache.loading = None;
            cache.loaded = Some(
                result
                    .map_err(|_| CoreError::Unavailable("knowledge load task failed".into()))??,
            );
        }
    }
}

#[async_trait::async_trait]
impl KnowledgeSource for Knowledge {
    async fn pin(&self) -> CoreResult<IndexPin> {
        let stored = self
            .store
            .knowledge_snapshot(&self.configured_revision)
            .await?
            .ok_or_else(|| {
                CoreError::Unavailable("configured knowledge index is not stored".into())
            })?;
        Ok(IndexPin {
            publication: stored.publication,
            revision: stored.id,
        })
    }

    async fn query(
        &self,
        revision: &str,
        query: &str,
        ranking: SourceRanking,
    ) -> CoreResult<knowledge::index::SourceQueryResult> {
        let index = self.ranked_index(revision, ranking).await?;
        let query = query.to_owned();
        tokio::task::spawn_blocking(move || index.search(&query, None, 6))
            .await
            .map_err(|_| CoreError::Unavailable("knowledge search worker failed".into()))?
            .map_err(|e| CoreError::Unavailable(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn interrupted_reader_resumes_the_original_index_load() {
        let Ok(uri) = std::env::var("WISHPOOL_TEST_MONGODB_URI") else {
            return;
        };
        let database = format!(
            "mathesis_knowledge_cache_{}",
            mathesis_core::ids::new_uuid()
        );
        let store = MongoStore::connect(&uri, &database).await.unwrap();
        let publication = PublicationIdentity {
            bundle_digest: knowledge::sha256(b"cache fixture"),
            source_commit: "a".repeat(40),
            source_tree: "b".repeat(40),
        };
        let index = SourceIndex::build(publication, vec![], |_| unreachable!()).unwrap();
        let revision = index.revision.clone();
        let blob = store.put_knowledge_index(&index).await.unwrap();
        store
            .insert_knowledge_snapshot(&KnowledgeSnapshot {
                id: revision.clone(),
                publication: index.publication.clone(),
                index: blob,
                artifacts: Default::default(),
                candidate_modules: 0,
                indexed_modules: 0,
                omitted_modules: 0,
                admitted_at: chrono::Utc::now(),
                admission: serde_json::json!({"synthetic_test":true}),
            })
            .await
            .unwrap();
        let service = Knowledge::new(store.clone(), revision.clone());
        let release = Arc::new(tokio::sync::Notify::new());
        let ready = release.clone();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let called = calls.clone();
        service.cache.lock().await.loading = Some(tokio::spawn(async move {
            called.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            ready.notified().await;
            Ok(Arc::new(SearchIndex::new(index).unwrap()))
        }));
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(20),
                service.index(&revision)
            )
            .await
            .is_err()
        );
        assert!(service.cache.lock().await.loading.is_some());
        release.notify_one();
        let first = service.index(&revision).await.unwrap();
        let replay = service.index(&revision).await.unwrap();
        assert!(Arc::ptr_eq(&first, &replay));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        let ranked = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            service.ranked_index(&revision, SourceRanking::IdentifierPaths),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(ranked.identity(), first.identity());
        assert_eq!(
            ranked.ranking_protocol(),
            SourceRanking::IdentifierPaths.protocol()
        );
        assert!(!Arc::ptr_eq(&first, &ranked));
        let old_again = service.index(&revision).await.unwrap();
        assert_eq!(old_again.ranking_protocol(), None);
        assert_eq!(old_again.identity(), first.identity());
        let client = mongodb::Client::with_uri_str(&uri).await.unwrap();
        client.database(&database).drop().await.unwrap();
        client.shutdown().await;
        store.shutdown().await;
    }
}
