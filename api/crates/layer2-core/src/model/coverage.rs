//! Downstream knowledge observations. Query execution, mathematical relation
//! and evidence qualification are independent; none is a novelty certificate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::require_text;
use crate::{
    CoreError, CoreResult,
    ids::{ClaimId, PersonId, SubmissionId},
};

#[cfg(test)]
mod tests;

pub const KNOWLEDGE_QUERY_PROTOCOL: &str = "mathesis.artifact-query.v1";
pub const KNOWLEDGE_ORACLE_SCOPE: &str = "mathesis.semantic-query.v1:oracle";
pub const KNOWLEDGE_CMA_SCOPE: &str = "mathesis.semantic-query.v1:cma";
pub const KNOWLEDGE_RANKER_SCOPE: &str = "mathesis.source-ranker.identifier-paths.v1";

/// Ranker identity is part of each digest-bound request, including failed ones.
pub fn validate_knowledge_ranker(contract: u32, scopes: &[String]) -> CoreResult<()> {
    let rankers = scopes
        .iter()
        .filter(|s| s.starts_with("mathesis.source-ranker."))
        .collect::<Vec<_>>();
    if (contract >= 3 && (rankers.len() != 1 || rankers[0].as_str() != KNOWLEDGE_RANKER_SCOPE))
        || (contract < 3 && !rankers.is_empty())
    {
        return Err(CoreError::invalid(
            "knowledge query ranker differs from its round contract",
        ));
    }
    Ok(())
}

fn digest(value: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(value))
}

fn require_digest(value: &str) -> CoreResult<()> {
    if value.strip_prefix("sha256:").is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    }) {
        Ok(())
    } else {
        Err(CoreError::invalid("invalid knowledge digest"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSubject {
    pub submission: SubmissionId,
    pub version: u32,
    pub claims_revision: u64,
    pub round: u32,
    pub claim: ClaimId,
    pub source_digest: String,
    pub claim_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeQueryIntent {
    pub id: String,
    pub subject: CoverageSubject,
    pub snapshot: Option<String>,
    pub index_revision: Option<String>,
    pub request_digest: String,
    pub query: String,
    pub scopes: Vec<String>,
    pub admitted_at: DateTime<Utc>,
}

impl KnowledgeQueryIntent {
    pub fn validate(&self) -> CoreResult<()> {
        self.subject.validate()?;
        require_text("query intent id", &self.id, 200)?;
        require_text("query", &self.query, 30_000)?;
        if let Some(value) = &self.snapshot {
            require_digest(value)?;
        }
        if let Some(value) = &self.index_revision {
            require_digest(value)?;
        }
        if self.scopes.len() > 40
            || self.request_digest
                != KnowledgeQueryReceipt::request_digest(
                    &self.subject,
                    &self.snapshot,
                    &self.index_revision,
                    &self.query,
                    &self.scopes,
                )
        {
            return Err(CoreError::invalid("query intent does not bind its inputs"));
        }
        for scope in &self.scopes {
            require_text("query scope", scope, 3_000)?;
        }
        Ok(())
    }

    pub fn matches(&self, receipt: &KnowledgeQueryReceipt) -> bool {
        self.id == receipt.id
            && self.subject == receipt.subject
            && self.snapshot == receipt.snapshot
            && self.index_revision == receipt.index_revision
            && self.query == receipt.query
            && self.scopes == receipt.scopes
            && self.request_digest == receipt.request_digest
            && self.admitted_at <= receipt.started_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum KnowledgeEvent {
    QueryAdmitted {
        intent: KnowledgeQueryIntent,
    },
    QueryCompleted {
        receipt: KnowledgeQueryReceipt,
    },
    CoverageAssessed {
        assessment: ClaimCoverage,
        at: DateTime<Utc>,
    },
}

impl KnowledgeEvent {
    pub fn subject(&self) -> &CoverageSubject {
        match self {
            Self::QueryAdmitted { intent } => &intent.subject,
            Self::QueryCompleted { receipt } => &receipt.subject,
            Self::CoverageAssessed { assessment, .. } => &assessment.subject,
        }
    }
}

impl CoverageSubject {
    pub fn validate(&self) -> CoreResult<()> {
        if self.version == 0 || self.round == 0 {
            return Err(CoreError::invalid(
                "knowledge subject requires a version and round",
            ));
        }
        require_digest(&self.source_digest)?;
        require_digest(&self.claim_digest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeDeclarationRef {
    pub snapshot: String,
    pub module_path: String,
    pub declaration_name_key: String,
    pub statement_id: String,
    pub type_digest: String,
}

impl KnowledgeDeclarationRef {
    pub fn validate(&self) -> CoreResult<()> {
        require_digest(&self.snapshot)?;
        require_digest(&self.statement_id)?;
        require_digest(&self.type_digest)?;
        require_text("knowledge module", &self.module_path, 2_000)?;
        require_text("declaration name key", &self.declaration_name_key, 8_000)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeQueryState {
    Complete,
    Partial,
    Timeout,
    Unavailable,
    InvalidPublication,
}

/// Created by an actual system call, never deserialized from model output.
/// Full query text lives with the private review; public summaries omit it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeQueryReceipt {
    pub id: String,
    pub protocol: String,
    pub subject: CoverageSubject,
    pub snapshot: Option<String>,
    pub index_revision: Option<String>,
    pub request_digest: String,
    pub query: String,
    pub scopes: Vec<String>,
    pub state: KnowledgeQueryState,
    /// Exact bounded adapter output, private GridFS payload. Hash and size bind
    /// the actual source excerpts supplied to the reviewers, not just hit ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<super::BlobRef>,
    pub returned: Vec<KnowledgeDeclarationRef>,
    pub limits: Vec<String>,
    pub retryable: bool,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

impl KnowledgeQueryReceipt {
    /// Caller assigns a durable request identity before external work. A digest
    /// alone is not an authorization mechanism or proof of successful execution.
    pub fn request_digest(
        subject: &CoverageSubject,
        snapshot: &Option<String>,
        index_revision: &Option<String>,
        query: &str,
        scopes: &[String],
    ) -> String {
        digest(
            &serde_json::to_vec(&(subject, snapshot, index_revision, query, scopes))
                .expect("knowledge request consists of serializable fields"),
        )
    }

    pub fn validate(&self) -> CoreResult<()> {
        self.subject.validate()?;
        require_text("query receipt id", &self.id, 200)?;
        require_text("private knowledge query", &self.query, 30_000)?;
        require_digest(&self.request_digest)?;
        if self.protocol != KNOWLEDGE_QUERY_PROTOCOL
            || self.completed_at < self.started_at
            || self.returned.len() > 100
            || self.scopes.len() > 40
            || self.limits.len() > 40
            || self.request_digest
                != Self::request_digest(
                    &self.subject,
                    &self.snapshot,
                    &self.index_revision,
                    &self.query,
                    &self.scopes,
                )
        {
            return Err(CoreError::invalid("invalid knowledge query receipt"));
        }
        if let Some(snapshot) = &self.snapshot {
            require_digest(snapshot)?;
        }
        if let Some(index) = &self.index_revision {
            require_digest(index)?;
        }
        if matches!(
            self.state,
            KnowledgeQueryState::Complete | KnowledgeQueryState::Partial
        ) && (self.snapshot.is_none()
            || self.index_revision.is_none()
            || self.scopes.is_empty()
            || self.response.is_none())
        {
            return Err(CoreError::invalid(
                "completed query needs a snapshot, index, scope and exact response",
            ));
        }
        if self.state != KnowledgeQueryState::Complete && self.limits.is_empty() {
            return Err(CoreError::invalid(
                "incomplete query needs an explicit limit",
            ));
        }
        if matches!(
            self.state,
            KnowledgeQueryState::Timeout
                | KnowledgeQueryState::Unavailable
                | KnowledgeQueryState::InvalidPublication
        ) && (!self.returned.is_empty() || self.response.is_some())
        {
            return Err(CoreError::invalid(
                "failed query cannot present successful results; use partial",
            ));
        }
        if let Some(response) = &self.response {
            require_text("query response blob", &response.id, 200)?;
            require_digest(&format!("sha256:{}", response.sha256))?;
            if response.bytes == 0 || response.bytes > 2 * 1024 * 1024 {
                return Err(CoreError::invalid("query response exceeds its byte limit"));
            }
        }
        for text in self.scopes.iter().chain(&self.limits) {
            require_text("query scope or limit", text, 3_000)?;
        }
        let mut returned = std::collections::BTreeSet::new();
        for reference in &self.returned {
            reference.validate()?;
            if self.snapshot.as_ref() != Some(&reference.snapshot)
                || !returned.insert(&reference.statement_id)
            {
                return Err(CoreError::invalid(
                    "query result belongs to another snapshot",
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageRelation {
    Unknown,
    Candidate,
    Covered,
    Refuted,
}

/// Projection into the pure publication policy. The application derives this
/// from durable query receipts and the validated audit; models cannot set it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeAdmission {
    pub receipt_ids: Vec<String>,
    pub complete: bool,
    pub audited_relation: Option<CoverageRelation>,
}

impl KnowledgeAdmission {
    pub fn sufficient(&self) -> bool {
        self.complete && !self.receipt_ids.is_empty() && self.audited_relation.is_some()
    }

    pub fn permits_new_content(&self) -> bool {
        self.sufficient()
            && matches!(
                self.audited_relation,
                Some(CoverageRelation::Unknown | CoverageRelation::Candidate)
            )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingRelation {
    Equivalent,
    Instance,
    Implies,
    Refutes,
    ProofUse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HypothesisDischarge {
    pub hypothesis: String,
    /// Empty means an unresolved obligation, never an automatically true premise.
    pub argument: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterBinding {
    pub parameter: String,
    pub expression: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeBinding {
    /// Joint premise set: no individual edge is inferred from this list.
    pub premises: Vec<KnowledgeDeclarationRef>,
    pub relation: BindingRelation,
    pub parameters: Vec<ParameterBinding>,
    pub hypotheses: Vec<HypothesisDischarge>,
    pub argument: String,
    pub unresolved: Vec<String>,
    /// Original submission and every known derivative; retained under reuse.
    pub source_lineage: Vec<SubmissionId>,
}

impl KnowledgeBinding {
    fn validate(&self, subject: &CoverageSubject) -> CoreResult<()> {
        if self.premises.is_empty()
            || self.premises.len() > 40
            || self.parameters.len() > 80
            || self.hypotheses.len() > 80
            || self.unresolved.len() > 40
            || self.source_lineage.len() > 100
            || self.source_lineage.contains(&subject.submission)
        {
            return Err(CoreError::invalid(
                "invalid or self-supporting knowledge binding",
            ));
        }
        let mut premises = std::collections::BTreeSet::new();
        for premise in &self.premises {
            premise.validate()?;
            if !premises.insert(&premise.statement_id) {
                return Err(CoreError::invalid("duplicate binding premise"));
            }
        }
        for parameter in &self.parameters {
            require_text("binding parameter", &parameter.parameter, 2_000)?;
            require_text("parameter expression", &parameter.expression, 5_000)?;
        }
        for hypothesis in &self.hypotheses {
            require_text("required hypothesis", &hypothesis.hypothesis, 5_000)?;
            if hypothesis.argument.len() > 10_000 {
                return Err(CoreError::invalid("hypothesis argument too large"));
            }
        }
        for obligation in &self.unresolved {
            require_text("remaining obligation", obligation, 5_000)?;
        }
        require_text("binding argument", &self.argument, 20_000)
    }

    pub fn discharged(&self) -> bool {
        self.unresolved.is_empty()
            && self
                .hypotheses
                .iter()
                .all(|h| !h.argument.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum CoverageQualification {
    Candidate,
    SourceAudited {
        auditor: PersonId,
        audit_id: String,
    },
    /// Stored as a claim until a recognized checker is actually configured.
    CertificateClaim {
        protocol: String,
        artifact_digest: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimCoverage {
    pub subject: CoverageSubject,
    pub receipt_ids: Vec<String>,
    pub relation: CoverageRelation,
    pub qualification: CoverageQualification,
    pub binding: Option<KnowledgeBinding>,
    pub summary: String,
    pub limits: Vec<String>,
}

impl ClaimCoverage {
    pub fn validate(&self, receipts: &[KnowledgeQueryReceipt]) -> CoreResult<()> {
        self.subject.validate()?;
        require_text("coverage summary", &self.summary, 10_000)?;
        for limit in &self.limits {
            require_text("coverage limit", limit, 3_000)?;
        }
        if self.receipt_ids.is_empty() || self.receipt_ids.len() > 40 || self.limits.len() > 40 {
            return Err(CoreError::invalid(
                "coverage needs actual bounded query references",
            ));
        }
        let mut matched = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for id in &self.receipt_ids {
            if !ids.insert(id) {
                return Err(CoreError::invalid("duplicate coverage receipt"));
            }
            let receipt = receipts
                .iter()
                .find(|r| &r.id == id)
                .ok_or_else(|| CoreError::invalid("coverage names a nonexistent query receipt"))?;
            receipt.validate()?;
            if receipt.subject != self.subject {
                return Err(CoreError::invalid(
                    "coverage receipt belongs to another version or claim",
                ));
            }
            matched.push(receipt);
        }
        let snapshots: std::collections::BTreeSet<_> = matched
            .iter()
            .filter_map(|r| r.snapshot.as_ref().zip(r.index_revision.as_ref()))
            .collect();
        if snapshots.len() > 1 {
            return Err(CoreError::invalid(
                "a coverage comparison cannot mix snapshots or index revisions",
            ));
        }
        if let Some(binding) = &self.binding {
            binding.validate(&self.subject)?;
            for premise in &binding.premises {
                if !matched.iter().any(|r| r.returned.contains(premise)) {
                    return Err(CoreError::invalid(
                        "binding premise was not returned by a referenced query",
                    ));
                }
            }
        }
        if matches!(
            self.relation,
            CoverageRelation::Covered | CoverageRelation::Refuted
        ) {
            let binding = self
                .binding
                .as_ref()
                .ok_or_else(|| CoreError::invalid("relation needs an argument"))?;
            if !binding.discharged()
                || binding.relation == BindingRelation::ProofUse
                || (self.relation == CoverageRelation::Refuted)
                    != (binding.relation == BindingRelation::Refutes)
                || !matches!(&self.qualification, CoverageQualification::SourceAudited { audit_id, .. }
                    if !audit_id.trim().is_empty())
            {
                return Err(CoreError::invalid(
                    "relation lacks discharged, source-audited evidence",
                ));
            }
        }
        // No upstream coverage-certificate protocol is admitted yet. This enum
        // preserves the submitted claim without creating certified evidence.
        if let CoverageQualification::CertificateClaim {
            artifact_digest,
            protocol,
        } = &self.qualification
        {
            require_digest(artifact_digest)?;
            require_text("claimed certificate protocol", protocol, 300)?;
        }
        Ok(())
    }

    pub fn required_query_complete(&self, receipts: &[KnowledgeQueryReceipt]) -> bool {
        self.validate(receipts).is_ok()
            && self.receipt_ids.iter().all(|id| {
                receipts
                    .iter()
                    .any(|r| &r.id == id && r.state == KnowledgeQueryState::Complete)
            })
    }
}
