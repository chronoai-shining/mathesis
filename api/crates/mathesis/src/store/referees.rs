use async_trait::async_trait;
use mathesis_core::{
    CoreError, CoreResult,
    ids::SubmissionId,
    model::{BlobRef, RefereeFile},
    ports::{BlobStore, RefereeStore},
};
use mongodb::bson::{Document, doc};
use sha2::{Digest, Sha256};

use super::{MongoStore, REFEREE_FILES, from_document, to_document, unavailable, write_error};

const INLINE_BYTES: usize = 512 * 1024;

impl MongoStore {
    async fn referee_document(
        &self,
        file: &RefereeFile,
    ) -> CoreResult<(Document, Option<BlobRef>)> {
        let bytes = serde_json::to_vec(file)
            .map_err(|error| CoreError::Unavailable(format!("encode review history: {error}")))?;
        if bytes.len() <= INLINE_BYTES {
            return Ok((to_document(file)?, None));
        }
        let snapshot =
            BlobStore::put(self, &bytes, "application/vnd.mathesis.review-history+json").await?;
        let document = doc! {
            "id": file.id.as_str(),
            "revision": file.revision as i64,
            "storage_format": "gridfs-json-v1",
            "snapshot": to_document(&snapshot)?,
        };
        Ok((document, Some(snapshot)))
    }

    async fn discard_referee_candidate(&self, snapshot: Option<BlobRef>) {
        if let Some(snapshot) = snapshot {
            let _ = BlobStore::discard(self, &snapshot.id).await;
        }
    }
}

#[async_trait]
impl RefereeStore for MongoStore {
    async fn get(&self, id: &SubmissionId) -> CoreResult<Option<RefereeFile>> {
        let Some(document) = self
            .raw(REFEREE_FILES)
            .find_one(doc! { "id": id.as_str() })
            .await
            .map_err(unavailable)?
        else {
            return Ok(None);
        };
        if !document.contains_key("storage_format") {
            return from_document(document).map(Some);
        }
        if document.get_str("storage_format") != Ok("gridfs-json-v1") {
            return Err(CoreError::Unavailable(
                "unknown review history storage format".into(),
            ));
        }
        let snapshot: BlobRef = from_document(
            document
                .get_document("snapshot")
                .map_err(|_| CoreError::Unavailable("review history reference missing".into()))?
                .clone(),
        )?;
        let bytes = BlobStore::get(self, &snapshot.id)
            .await?
            .ok_or_else(|| CoreError::Unavailable("review history snapshot missing".into()))?;
        if bytes.len() as u64 != snapshot.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != snapshot.sha256
        {
            return Err(CoreError::Unavailable(
                "review history snapshot checksum mismatch".into(),
            ));
        }
        let file: RefereeFile = serde_json::from_slice(&bytes)
            .map_err(|_| CoreError::Unavailable("invalid review history snapshot".into()))?;
        if &file.id != id || document.get_i64("revision").ok() != Some(file.revision as i64) {
            return Err(CoreError::Unavailable(
                "review history snapshot identity mismatch".into(),
            ));
        }
        Ok(Some(file))
    }

    async fn insert(&self, file: &RefereeFile) -> CoreResult<()> {
        let (document, snapshot) = self.referee_document(file).await?;
        match self.raw(REFEREE_FILES).insert_one(document).await {
            Ok(_) => Ok(()),
            Err(error) => {
                let error = write_error(error);
                if matches!(error, CoreError::Conflict(_)) {
                    self.discard_referee_candidate(snapshot).await;
                }
                Err(error)
            }
        }
    }

    async fn replace(&self, file: &RefereeFile, expected: u64) -> CoreResult<()> {
        let mut file = file.clone();
        file.revision = expected + 1;
        let (document, snapshot) = self.referee_document(&file).await?;
        let result = self
            .raw(REFEREE_FILES)
            .replace_one(
                doc! { "id": file.id.as_str(), "revision": expected as i64 },
                document,
            )
            .await
            .map_err(write_error)?;
        if result.matched_count == 1 {
            return Ok(());
        }
        self.discard_referee_candidate(snapshot).await;
        match self
            .raw(REFEREE_FILES)
            .find_one(doc! { "id": file.id.as_str() })
            .await
            .map_err(unavailable)?
        {
            Some(_) => Err(CoreError::StaleRevision {
                kind: "referee file",
                id: file.id.to_string(),
            }),
            None => Err(CoreError::not_found("referee file", file.id.as_str())),
        }
    }
}
