//! Authorized downstream proposal packages. A package and an asserted upstream
//! reference never establish acceptance, freezing or mathematical equivalence.
use super::{BlobRef, KnowledgeDeclarationRef, require_text};
use crate::{
    CoreError, CoreResult,
    ids::{ClaimId, PersonId, RecordId, SubmissionId},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const FEEDBACK_PREPARATION_CONSENT: &str = "feedback-prepare-2026-10-16";
pub const FEEDBACK_EXPORT_CONSENT: &str = "feedback-export-2026-10-16";
pub const FEEDBACK_LICENSE: &str = "Apache-2.0";
pub const FEEDBACK_PROTOCOL: &str = "mathesis.feedback-proposal.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FourSlotProposal {
    pub escape_from: String,
    pub readout_via: String,
    pub realization: String,
    pub escape_continues: String,
}
impl FourSlotProposal {
    pub fn validate(&self) -> CoreResult<()> {
        for text in [
            &self.escape_from,
            &self.readout_via,
            &self.realization,
            &self.escape_continues,
        ] {
            require_text("proposed registration slot", text, 5000)?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FeedbackKind {
    OpenProblem {
        claim: ClaimId,
    },
    VerifiedSolution {
        claim: ClaimId,
        attempt: String,
    },
    LibraryBinding {
        board: String,
        contribution: String,
    },
    WitnessFormalization {
        claim: ClaimId,
        witness: usize,
    },
    DtrRegistration {
        claim: ClaimId,
        attempt: String,
        slots: FourSlotProposal,
    },
    Erratum {
        report: String,
        upstream: Vec<KnowledgeDeclarationRef>,
    },
}
impl FeedbackKind {
    pub fn validate(&self) -> CoreResult<()> {
        match self {
            Self::OpenProblem { .. } | Self::WitnessFormalization { .. } => {}
            Self::VerifiedSolution { attempt, .. } => {
                require_text("verified attempt", attempt, 200)?
            }
            Self::LibraryBinding {
                board,
                contribution,
            } => {
                require_text("binding summary", board, 200)?;
                require_text("binding contribution", contribution, 200)?;
            }
            Self::DtrRegistration { attempt, slots, .. } => {
                require_text("verified attempt", attempt, 200)?;
                slots.validate()?;
            }
            Self::Erratum { report, upstream } => {
                require_text("confirmed error report", report, 200)?;
                if upstream.is_empty() || upstream.len() > 20 {
                    return Err(CoreError::invalid(
                        "erratum needs 1–20 explicit upstream source identities",
                    ));
                }
                for d in upstream {
                    d.validate()?;
                }
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackSource {
    pub submission: SubmissionId,
    pub record: RecordId,
    pub version: u32,
    pub claims_revision: u64,
    pub source_digest: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct FeedbackPreview {
    pub digest: String,
    pub producer: String,
    pub source: FeedbackSource,
    pub kind: FeedbackKind,
    pub required_signers: Vec<PersonId>,
    pub license: String,
    pub payload: serde_json::Value,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewFeedbackProposal {
    pub kind: FeedbackKind,
    pub preview_digest: String,
    pub preparation_consent: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackAuthorization {
    pub digest: String,
    pub license: String,
    pub consent: String,
    pub rights_asserted: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackReadbackRequest {
    pub pull_request: u64,
    pub index_revision: String,
    pub declarations: Vec<KnowledgeDeclarationRef>,
    /// A caller-chosen key permits an explicit later observation of the same PR.
    pub observation_key: String,
}
impl FeedbackReadbackRequest {
    pub fn validate(&self) -> CoreResult<()> {
        if self.pull_request == 0
            || self.pull_request > 100_000_000
            || self.declarations.is_empty()
            || self.declarations.len() > 20
        {
            return Err(CoreError::invalid(
                "readback requires a PR and 1–20 exact declaration identities",
            ));
        }
        require_text("observation key", &self.observation_key, 100)?;
        if !self
            .index_revision
            .strip_prefix("sha256:")
            .is_some_and(|s| {
                s.len() == 64
                    && s.bytes()
                        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            })
        {
            return Err(CoreError::invalid("invalid publication index revision"));
        }
        for (i, d) in self.declarations.iter().enumerate() {
            d.validate()?;
            if self.declarations[..i].contains(d) {
                return Err(CoreError::invalid(
                    "duplicate declaration in observation request",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackDeclarationObservation {
    pub identity: KnowledgeDeclarationRef,
    pub present: bool,
    pub source_sha256: Option<String>,
    pub freeze_status: Option<String>,
    pub frozen_node_id: Option<String>,
    /// Exact package digest found in the actual full admitted module source.
    /// This is an explicit provenance link, never a semantic equivalence proof.
    pub package_marker_present: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackObservation {
    pub request: String,
    pub pull_request: u64,
    pub upstream_repository: String,
    pub upstream_base: String,
    pub reference_state: String,
    pub merge_commit: Option<String>,
    pub merged_at: Option<DateTime<Utc>>,
    pub consumed_source_commit: String,
    pub consumed_source_tree: String,
    pub snapshot: String,
    pub index_revision: String,
    pub merge_in_consumed_source: bool,
    pub declarations: Vec<FeedbackDeclarationObservation>,
    pub evidence: BlobRef,
    pub observed_at: DateTime<Utc>,
    pub limits: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum FeedbackEvent {
    Authorized {
        by: PersonId,
        digest: String,
        license: String,
        consent: String,
        rights_asserted: bool,
        at: DateTime<Utc>,
    },
    Revoked {
        by: PersonId,
        reason: String,
        at: DateTime<Utc>,
    },
    Exported {
        by: PersonId,
        at: DateTime<Utc>,
    },
    ReadbackRequested {
        id: String,
        request: FeedbackReadbackRequest,
        by: PersonId,
        at: DateTime<Utc>,
    },
    Observed {
        observation: Box<FeedbackObservation>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedbackProposal {
    pub id: String,
    pub digest: String,
    pub producer: String,
    pub source: FeedbackSource,
    pub kind: FeedbackKind,
    pub payload: BlobRef,
    pub required_signers: Vec<PersonId>,
    pub prepared_by: PersonId,
    pub prepared_at: DateTime<Utc>,
    pub preparation_consent: String,
    pub events: Vec<FeedbackEvent>,
}
impl FeedbackProposal {
    pub fn revoked(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, FeedbackEvent::Revoked { .. }))
    }
    pub fn authorized(&self) -> bool {
        !self.revoked()&&!self.required_signers.is_empty()&&self.required_signers.iter().all(|person|self.events.iter().any(|e|matches!(e,FeedbackEvent::Authorized{by,digest,license,consent,rights_asserted:true,..} if by==person&&digest==&self.digest&&license==FEEDBACK_LICENSE&&consent==FEEDBACK_EXPORT_CONSENT)))
    }
    pub fn pending_readback(&self) -> Option<(String, FeedbackReadbackRequest)> {
        self.events.iter().find_map(|e|match e{FeedbackEvent::ReadbackRequested{id,request,..} if !self.events.iter().any(|e|matches!(e,FeedbackEvent::Observed{observation} if observation.request==*id))=>Some((id.clone(),request.clone())),_=>None})
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct FeedbackView {
    pub proposal: FeedbackProposal,
    pub payload: Option<serde_json::Value>,
    pub state: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct FeedbackExport {
    pub protocol: String,
    pub proposal: String,
    pub digest: String,
    pub payload: serde_json::Value,
    pub authorizations: Vec<FeedbackEvent>,
    pub proposal_markdown: String,
}
/// Composition-only result of actual read-only observations; no public handler
/// accepts it. Exact response bytes are persisted separately by the application.
pub struct InspectedFeedback {
    pub observation: FeedbackObservation,
    pub evidence: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct PendingFeedback {
    pub proposal: FeedbackProposal,
    pub request_id: String,
    pub request: FeedbackReadbackRequest,
}
