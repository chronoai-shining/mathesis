//! Error reports are private until confirmed; quotas are fenced per entrant.
use super::{Entrant, VerificationReceipt};
use crate::ids::{ClaimId, PersonId, RecordId, SubmissionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ErrorSubject {
    Statement { claim: ClaimId },
    Target { claim: ClaimId, digest: String },
    Review,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorVerdict {
    Confirmed,
    Refuted,
    Unclear,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResolution {
    pub verdict: ErrorVerdict,
    pub reason: String,
    pub evidence: String,
    pub at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorReport {
    pub id: String,
    pub record: RecordId,
    pub submission: SubmissionId,
    pub version: u32,
    pub claims_revision: u64,
    pub reporter: Entrant,
    pub owner: PersonId,
    pub subject: ErrorSubject,
    pub text: String,
    pub solution: Option<String>,
    pub attempt: Option<String>,
    pub receipt: Option<VerificationReceipt>,
    pub resolution: Option<ErrorResolution>,
    pub effects_applied: bool,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorReportFile {
    pub id: String,
    pub revision: u64,
    pub reports: Vec<ErrorReport>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewErrorReport {
    pub subject: ErrorSubject,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub solution: Option<String>,
    #[serde(default)]
    pub as_agent: Option<String>,
}
/// Explicit allowlist: never serialize the stored aggregate to a reader.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorReportView {
    pub id: String,
    pub record: RecordId,
    pub reporter: Entrant,
    pub subject: ErrorSubject,
    pub text: String,
    pub resolution: Option<ErrorResolution>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<VerificationReceipt>,
}
