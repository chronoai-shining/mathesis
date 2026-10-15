//! Immutable, operator-admitted public knowledge. The index is a rebuildable
//! projection; checked bytes and publication provenance remain in GridFS.

use std::collections::BTreeMap;

use futures::io::{AsyncReadExt, AsyncWriteExt};
use mathesis_core::{CoreError, CoreResult, ids::new_uuid, model::BlobRef, ports::BlobStore};
use mathesis_review::knowledge::{PublicationIdentity, index::SourceIndex, sha256};
use mongodb::bson::{Bson, doc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::AsyncReadExt as _;

use super::{MongoStore, from_document, is_duplicate_key, to_document, unavailable};

pub(crate) const KNOWLEDGE_SNAPSHOTS: &str = "knowledge_snapshots";
pub(crate) const MAX_INDEX_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct KnowledgeSnapshot {
    pub id: String,
    pub publication: PublicationIdentity,
    pub index: BlobRef,
    pub artifacts: BTreeMap<String, BlobRef>,
    pub candidate_modules: usize,
    pub indexed_modules: usize,
    pub omitted_modules: usize,
    pub admitted_at: chrono::DateTime<chrono::Utc>,
    /// Operator evidence is retained verbatim, never re-labelled as a signed
    /// upstream attestation or a model's confirmation of provenance.
    pub admission: serde_json::Value,
}

impl MongoStore {
    pub(crate) async fn verify_knowledge_blob(&self, blob: &BlobRef) -> CoreResult<()> {
        if blob.bytes > 4 * 1024 * 1024 * 1024 {
            return Err(CoreError::Unavailable(
                "knowledge artifact exceeds verification limit".into(),
            ));
        }
        let mut download = self
            .bucket()
            .open_download_stream(Bson::String(blob.id.clone()))
            .await
            .map_err(unavailable)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0_u8; 256 * 1024];
        let mut count = 0_u64;
        loop {
            let read = download
                .read(&mut buffer)
                .await
                .map_err(|_| CoreError::Unavailable("cannot verify knowledge artifact".into()))?;
            if read == 0 {
                break;
            }
            count += read as u64;
            if count > blob.bytes {
                return Err(CoreError::Unavailable(
                    "knowledge artifact length mismatch".into(),
                ));
            }
            hasher.update(&buffer[..read]);
        }
        if count != blob.bytes || format!("{:x}", hasher.finalize()) != blob.sha256 {
            return Err(CoreError::Unavailable(
                "knowledge artifact checksum mismatch".into(),
            ));
        }
        Ok(())
    }

    pub(crate) async fn knowledge_snapshot(
        &self,
        revision: &str,
    ) -> CoreResult<Option<KnowledgeSnapshot>> {
        self.get_by_id(KNOWLEDGE_SNAPSHOTS, revision).await
    }

    pub(crate) async fn load_knowledge_index(&self, revision: &str) -> CoreResult<SourceIndex> {
        let snapshot = self
            .knowledge_snapshot(revision)
            .await?
            .ok_or_else(|| CoreError::Unavailable("pinned knowledge index is not stored".into()))?;
        let bytes = self
            .read_checked_blob(&snapshot.index, MAX_INDEX_BYTES)
            .await?;
        let index: SourceIndex = serde_json::from_slice(&bytes)
            .map_err(|_| CoreError::Unavailable("invalid stored knowledge index".into()))?;
        index
            .validate()
            .map_err(|e| CoreError::Unavailable(e.to_string()))?;
        if index.revision != revision
            || index.publication != snapshot.publication
            || index.candidate_module_count != snapshot.candidate_modules
            || index.modules.len() != snapshot.indexed_modules
            || index.omissions.len() != snapshot.omitted_modules
        {
            return Err(CoreError::Unavailable(
                "knowledge snapshot header mismatch".into(),
            ));
        }
        Ok(index)
    }

    pub(crate) async fn insert_knowledge_snapshot(
        &self,
        snapshot: &KnowledgeSnapshot,
    ) -> CoreResult<KnowledgeSnapshot> {
        match self
            .raw(KNOWLEDGE_SNAPSHOTS)
            .insert_one(to_document(snapshot)?)
            .await
        {
            Ok(_) => Ok(snapshot.clone()),
            Err(e) if is_duplicate_key(&e) => {
                let prior = self
                    .raw(KNOWLEDGE_SNAPSHOTS)
                    .find_one(doc! {"id": &snapshot.id})
                    .await
                    .map_err(unavailable)?
                    .map(from_document::<KnowledgeSnapshot>)
                    .transpose()?
                    .ok_or_else(|| {
                        CoreError::Unavailable(
                            "knowledge insert conflict without saved snapshot".into(),
                        )
                    })?;
                let same_artifacts = prior.artifacts.len() == snapshot.artifacts.len()
                    && prior.artifacts.iter().all(|(k, v)| {
                        snapshot
                            .artifacts
                            .get(k)
                            .is_some_and(|b| b.sha256 == v.sha256 && b.bytes == v.bytes)
                    });
                if prior.publication != snapshot.publication
                    || prior.index.sha256 != snapshot.index.sha256
                    || prior.index.bytes != snapshot.index.bytes
                    || !same_artifacts
                {
                    return Err(CoreError::conflict(
                        "knowledge revision already has different immutable contents",
                    ));
                }
                // Never delete after uncertain writes. Confirmed duplicate uploads are
                // harmless orphans and can be collected by the existing offline audit.
                Ok(prior)
            }
            Err(e) => Err(unavailable(e)),
        }
    }

    pub(crate) async fn read_checked_blob(
        &self,
        blob: &BlobRef,
        maximum: u64,
    ) -> CoreResult<Vec<u8>> {
        if blob.bytes > maximum {
            return Err(CoreError::Unavailable(
                "stored knowledge exceeds read limit".into(),
            ));
        }
        let download = self
            .bucket()
            .open_download_stream(Bson::String(blob.id.clone()))
            .await
            .map_err(unavailable)?;
        let mut bytes = Vec::new();
        download
            .take(blob.bytes + 1)
            .read_to_end(&mut bytes)
            .await
            .map_err(|_| CoreError::Unavailable("cannot read stored knowledge".into()))?;
        if bytes.len() as u64 != blob.bytes || sha256(&bytes) != format!("sha256:{}", blob.sha256) {
            return Err(CoreError::Unavailable(
                "stored knowledge checksum mismatch".into(),
            ));
        }
        Ok(bytes)
    }

    /// Stream large public archives; never hold the canonical expression ZIP in
    /// memory. Verify the actual uploaded bytes against the admitted digest.
    pub(crate) async fn put_knowledge_file(
        &self,
        path: &std::path::Path,
        expected: &str,
        maximum: u64,
    ) -> CoreResult<BlobRef> {
        let mut file = tokio::fs::File::open(path)
            .await
            .map_err(|_| CoreError::Unavailable("cannot open knowledge artifact".into()))?;
        let id = new_uuid();
        let mut upload = self
            .bucket()
            .open_upload_stream(&id)
            .id(Bson::String(id.clone()))
            .metadata(doc! {"content_type":"application/octet-stream"})
            .await
            .map_err(unavailable)?;
        let mut hasher = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = vec![0_u8; 256 * 1024];
        let result: CoreResult<()> = async {
            loop {
                let count = file
                    .read(&mut buffer)
                    .await
                    .map_err(|_| CoreError::Unavailable("knowledge artifact read failed".into()))?;
                if count == 0 {
                    break;
                }
                bytes += count as u64;
                if bytes > maximum {
                    return Err(CoreError::invalid(
                        "knowledge artifact exceeds upload limit",
                    ));
                }
                hasher.update(&buffer[..count]);
                upload.write_all(&buffer[..count]).await.map_err(|_| {
                    CoreError::Unavailable("knowledge artifact upload failed".into())
                })?;
            }
            Ok(())
        }
        .await;
        let hash = format!("{:x}", hasher.finalize());
        if result.is_err() || format!("sha256:{hash}") != expected {
            let _ = upload.abort().await;
            result?;
            return Err(CoreError::invalid(
                "knowledge artifact changed before upload",
            ));
        }
        upload.close().await.map_err(|_| {
            CoreError::Unavailable("knowledge artifact upload completion uncertain".into())
        })?;
        Ok(BlobRef {
            id,
            bytes,
            sha256: hash,
        })
    }

    pub(crate) async fn put_knowledge_index(&self, index: &SourceIndex) -> CoreResult<BlobRef> {
        index
            .validate()
            .map_err(|e| CoreError::invalid(e.to_string()))?;
        let bytes = serde_json::to_vec(index)
            .map_err(|_| CoreError::invalid("cannot encode knowledge index"))?;
        if bytes.len() as u64 > MAX_INDEX_BYTES {
            return Err(CoreError::invalid("knowledge index exceeds storage limit"));
        }
        self.put(&bytes, "application/json").await
    }
}
