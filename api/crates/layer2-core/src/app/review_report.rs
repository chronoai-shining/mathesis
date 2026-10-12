//! Portable review dossier: source binding plus the existing safe author projection.
use super::App;
use crate::{CoreResult, ids::SubmissionId, model::*};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReviewReport {
    pub schema_version: u32,
    pub service: &'static str,
    pub submission: SubmissionId,
    pub title: String,
    pub authors: Vec<Author>,
    pub version: u32,
    pub claims_revision: u64,
    pub source_sha256: String,
    pub status: SubmissionStatus,
    pub claims: Vec<Claim>,
    pub review: RefereeView,
}
impl App {
    pub async fn review_report(
        &self,
        caller: &Caller,
        id: &SubmissionId,
    ) -> CoreResult<ReviewReport> {
        let submission = self.submission(caller, id).await?;
        let file = self
            .ports
            .referees
            .get(id)
            .await?
            .unwrap_or_else(|| RefereeFile::new(id.clone()));
        let version = submission.current_version();
        Ok(ReviewReport {
            schema_version: 1,
            service: "Orchestra",
            submission: id.clone(),
            title: submission.title.clone(),
            authors: submission.authors.clone(),
            version: version.map_or(0, |v| v.number),
            source_sha256: version.map_or_else(String::new, |v| v.archive.sha256.clone()),
            claims_revision: submission.claims_revision,
            status: submission.status.clone(),
            claims: submission.claims.clone(),
            review: file.view(false),
        })
    }
}
