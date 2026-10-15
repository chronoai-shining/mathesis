use std::collections::BTreeSet;

use futures::TryStreamExt;
use mathesis_core::{CoreError, CoreResult, ports::BlobStore};
use mongodb::bson::{Bson, DateTime, doc};
use serde::Serialize;

use super::{MongoStore, REFEREE_FILES, to_document, unavailable};

#[derive(Debug, Serialize)]
pub struct StorageAudit {
    pub orphan_files: Vec<String>,
    pub orphan_bytes: u64,
    pub missing_files: Vec<String>,
    pub orphan_chunk_files: Vec<String>,
    pub orphan_chunks: u64,
    pub minimum_age_days: u32,
}

fn references(value: &Bson, found: &mut BTreeSet<String>) {
    match value {
        Bson::Document(document) => {
            if document.contains_key("bytes")
                && document.contains_key("sha256")
                && let Ok(id) = document.get_str("id")
            {
                found.insert(id.to_owned());
            }
            for value in document.values() {
                references(value, found);
            }
        }
        Bson::Array(values) => {
            for value in values {
                references(value, found);
            }
        }
        _ => {}
    }
}

impl MongoStore {
    pub async fn storage_audit(&self) -> CoreResult<StorageAudit> {
        let mut referenced = BTreeSet::new();
        for collection in self.db.list_collection_names().await.map_err(unavailable)? {
            if collection.starts_with("papers.") || collection.starts_with("system.") {
                continue;
            }
            let mut cursor = self
                .raw(&collection)
                .find(doc! {})
                .await
                .map_err(unavailable)?;
            while let Some(document) = cursor.try_next().await.map_err(unavailable)? {
                if collection == REFEREE_FILES && document.contains_key("storage_format") {
                    let id = document
                        .get_str("id")
                        .map_err(|_| {
                            CoreError::Unavailable("review history identity missing".into())
                        })?
                        .into();
                    // A present blob is not necessarily a readable history. Collection must
                    // stop on corruption before deleting any other recovery material.
                    let file = self.read_referee_document(&id, document.clone()).await?;
                    references(&Bson::Document(to_document(&file)?), &mut referenced);
                }
                references(&Bson::Document(document), &mut referenced);
            }
        }
        let mut stored = BTreeSet::new();
        let mut candidates = Vec::new();
        let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
        let mut cursor = self
            .raw("papers.files")
            .find(doc! {})
            .await
            .map_err(unavailable)?;
        while let Some(document) = cursor.try_next().await.map_err(unavailable)? {
            let Ok(id) = document.get_str("_id") else {
                continue;
            };
            stored.insert(id.to_owned());
            let old = document
                .get_datetime("uploadDate")
                .is_ok_and(|at| at.to_chrono() < cutoff);
            if !referenced.contains(id) && old {
                candidates.push((
                    id.to_owned(),
                    document.get_i64("length").unwrap_or(0).max(0) as u64,
                ));
            }
        }
        let mut orphan_chunk_files = Vec::new();
        let mut orphan_chunks = 0;
        let mut chunks = self.raw("papers.chunks").aggregate([
            doc! { "$lookup": { "from": "papers.files", "localField": "files_id", "foreignField": "_id", "as": "file" } },
            doc! { "$match": { "file": { "$size": 0 } } },
            doc! { "$group": { "_id": "$files_id", "latest": { "$max": "$_id" }, "known_age": { "$min": { "$eq": [{ "$type": "$_id" }, "objectId"] } }, "count": { "$sum": 1 } } },
        ]).await.map_err(unavailable)?;
        while let Some(group) = chunks.try_next().await.map_err(unavailable)? {
            if group.get_bool("known_age") == Ok(true)
                && let (Ok(id), Ok(latest)) = (group.get_str("_id"), group.get_object_id("latest"))
                && latest.timestamp().to_chrono() < cutoff
                && !referenced.contains(id)
            {
                orphan_chunk_files.push(id.to_owned());
                orphan_chunks += group
                    .get_i32("count")
                    .map(i64::from)
                    .or_else(|_| group.get_i64("count"))
                    .map_err(|_| CoreError::Unavailable("invalid orphan chunk count".into()))?
                    as u64;
            }
        }
        Ok(StorageAudit {
            orphan_bytes: candidates.iter().map(|(_, bytes)| bytes).sum(),
            orphan_files: candidates.into_iter().map(|(id, _)| id).collect(),
            missing_files: referenced.difference(&stored).cloned().collect(),
            minimum_age_days: 7,
            orphan_chunk_files,
            orphan_chunks,
        })
    }

    pub async fn collect_orphan_files_offline(&self) -> CoreResult<StorageAudit> {
        if self
            .raw(super::REVIEW_JOBS)
            .find_one(doc! {
                "state": "leased", "lease_until": { "$gt": DateTime::now() }
            })
            .await
            .map_err(unavailable)?
            .is_some()
        {
            return Err(CoreError::conflict(
                "stop all API and worker instances and wait for existing leases before offline collection",
            ));
        }
        let audit = self.storage_audit().await?;
        if !audit.missing_files.is_empty() {
            return Err(CoreError::Unavailable(
                "referenced files are missing; restore storage before collecting orphans".into(),
            ));
        }
        for id in &audit.orphan_files {
            BlobStore::discard(self, id).await?;
        }
        for id in &audit.orphan_chunk_files {
            self.raw("papers.chunks")
                .delete_many(doc! { "files_id": id })
                .await
                .map_err(unavailable)?;
        }
        Ok(audit)
    }
}
