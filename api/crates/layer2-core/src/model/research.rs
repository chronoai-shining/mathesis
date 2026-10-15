//! Private, author-authorized mathematical obligations. A checked target is
//! internal evidence, never automatic external coverage or publication credit.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    Attempt, CoverageSubject, KnowledgeBinding, LeanStatementAttempt, LeanStatementResponse,
    VerificationReceipt, require_text,
};
use crate::{
    CoreError, CoreResult,
    ids::{ClaimId, PersonId},
};

pub const RESEARCH_CONSENT: &str = "research-obligation-2026-10-16";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ResearchGap {
    Hypothesis { index: usize },
    Bridge { index: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchGoal {
    pub definitions: Vec<String>,
    pub hypotheses: Vec<String>,
    pub quantifiers: String,
    pub conclusion: String,
    /// Why this exact goal addresses the selected audited obligation.
    pub correspondence: String,
}
impl ResearchGoal {
    pub fn validate(&self) -> CoreResult<()> {
        if self.definitions.len() > 30 || self.hypotheses.len() > 40 {
            return Err(CoreError::invalid("too many research goal premises"));
        }
        for item in self.definitions.iter().chain(&self.hypotheses) {
            require_text("research premise", item, 2_000)?;
        }
        require_text("goal quantifiers", &self.quantifiers, 6_000)?;
        require_text("goal conclusion", &self.conclusion, 6_000)?;
        require_text("goal correspondence", &self.correspondence, 6_000)?;
        if self.statement().len() > 30_000 {
            return Err(CoreError::invalid("research goal exceeds its byte budget"));
        }
        Ok(())
    }
    pub fn statement(&self) -> String {
        format!(
            "Definitions: {}\nHypotheses: {}\nQuantifiers: {}\nConclusion: {}",
            self.definitions.join("; "),
            self.hypotheses.join("; "),
            self.quantifiers,
            self.conclusion
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchBudget {
    pub max_attempts: u8,
    pub expires_at: DateTime<Utc>,
}
impl ResearchBudget {
    pub fn validate(&self, now: DateTime<Utc>) -> CoreResult<()> {
        if !(1..=5).contains(&self.max_attempts)
            || self.expires_at <= now
            || self.expires_at > now + chrono::Duration::days(30)
        {
            return Err(CoreError::invalid(
                "research needs 1–5 attempts and an expiry within 30 days",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewResearchObligation {
    pub round: u32,
    pub claim: ClaimId,
    pub gap: ResearchGap,
    /// Digest of the exact candidate displayed to the author.
    pub candidate_digest: String,
    pub goal: ResearchGoal,
    pub budget: ResearchBudget,
    pub consent: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchCandidate {
    pub digest: String,
    pub subject: CoverageSubject,
    pub gap: ResearchGap,
    pub unresolved_text: String,
    pub original_statement: String,
    pub receipt_ids: Vec<String>,
    pub binding: KnowledgeBinding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ResearchEvent {
    TargetPrepared {
        target: LeanStatementAttempt,
    },
    TargetDecided {
        digest: String,
        response: LeanStatementResponse,
    },
    AttemptAdmitted {
        attempt: Box<Attempt>,
    },
    AttemptChecked {
        attempt_id: String,
        receipt: VerificationReceipt,
    },
    AttemptInvalidated {
        attempt_id: String,
        reason: String,
    },
    Stopped {
        by: PersonId,
        reason: String,
        at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObligation {
    pub id: String,
    pub candidate: ResearchCandidate,
    pub goal: ResearchGoal,
    pub budget: ResearchBudget,
    pub consent: String,
    pub authorized_by: PersonId,
    pub authorized_at: DateTime<Utc>,
    pub events: Vec<ResearchEvent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResearchObligationView {
    pub target: Option<LeanStatementAttempt>,
    pub attempts: Vec<Attempt>,
    pub obligation: ResearchObligation,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResearchView {
    pub candidates: Vec<ResearchCandidate>,
    pub obligations: Vec<ResearchObligationView>,
}
impl ResearchObligation {
    pub fn stopped(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, ResearchEvent::Stopped { .. }))
    }
    pub fn target(&self) -> Option<LeanStatementAttempt> {
        let mut target = None;
        for event in &self.events {
            match event {
                ResearchEvent::TargetPrepared { target: saved } => target = Some(saved.clone()),
                ResearchEvent::TargetDecided { digest, response } => {
                    if let Some(current) = &mut target
                        && current.digest == *digest
                    {
                        current.response = response.clone();
                    }
                }
                _ => {}
            }
        }
        target
    }
    pub fn attempts(&self) -> Vec<Attempt> {
        let mut attempts = Vec::<Attempt>::new();
        for event in &self.events {
            match event {
                ResearchEvent::AttemptAdmitted { attempt } => {
                    attempts.push(attempt.as_ref().clone())
                }
                ResearchEvent::AttemptChecked {
                    attempt_id,
                    receipt,
                } => {
                    if let Some(attempt) = attempts.iter_mut().find(|a| a.id == *attempt_id) {
                        attempt.receipt = Some(receipt.clone());
                    }
                }
                ResearchEvent::AttemptInvalidated { attempt_id, reason } => {
                    if let Some(attempt) = attempts.iter_mut().find(|a| a.id == *attempt_id) {
                        attempt.invalidated = Some(reason.clone());
                    }
                }
                _ => {}
            }
        }
        attempts
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchAttemptInput {
    pub target_digest: String,
    pub solution: String,
    #[serde(default)]
    pub modules: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub note: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ResearchAttemptView {
    pub attempt: Attempt,
    pub solution: String,
    pub modules: std::collections::BTreeMap<String, String>,
}
/// Composition-only output of elaboration and an independent target inspection.
pub struct PreparedResearchTarget {
    pub previous_digest: Option<String>,
    pub lean: String,
    pub toolchain: String,
    pub reading: String,
    pub faithfulness: super::FaithfulnessCheck,
}
