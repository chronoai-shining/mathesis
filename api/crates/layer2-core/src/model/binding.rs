//! Consented public binding discussions. Source-audited judgments remain
//! distinct from verified proofs and recognized coverage certificates.
use super::*;
use crate::{CoreError, CoreResult, ids::PersonId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const BINDING_SHARING_CONSENT: &str = "binding-summary-2026-10-16";
pub const BINDING_CONTRIBUTION_CONSENT: &str = "binding-contribution-2026-10-16";
pub const BINDING_LICENSE: &str = "CC-BY-4.0";

pub(crate) fn binding_digest(value: &impl Serialize) -> String {
    format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(value).expect("binding data serializes"))
    )
}
fn normalized(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingTarget {
    pub subject: CoverageSubject,
    pub statement: String,
    pub sources: Vec<KnowledgeDeclarationRef>,
}
#[derive(Debug, Clone, Serialize)]
pub struct BindingOption {
    pub digest: String,
    pub target: BindingTarget,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewBindingBoard {
    pub option_digest: String,
    pub sources: Vec<KnowledgeDeclarationRef>,
    pub context: String,
    pub license: String,
    pub consent: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingBoard {
    pub id: String,
    pub target: BindingTarget,
    pub context: String,
    pub license: String,
    pub consent: String,
    pub authorized_by: PersonId,
    pub authorized_at: DateTime<Utc>,
    /// Private actual query identities. Never project query text publicly.
    pub receipt_ids: Vec<String>,
    pub events: Vec<BindingEvent>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewBindingContribution {
    pub binding: KnowledgeBinding,
    pub license: String,
    pub consent: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewBindingChallenge {
    pub contribution: String,
    pub evidence: String,
    pub license: String,
    pub consent: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingContribution {
    pub id: String,
    pub canonical: String,
    pub contributor: PersonId,
    pub binding: KnowledgeBinding,
    pub license: String,
    pub consent: String,
    pub at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingChallenge {
    pub id: String,
    pub contribution: String,
    pub challenger: PersonId,
    pub evidence: String,
    pub license: String,
    pub consent: String,
    pub at: DateTime<Utc>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingVerdict {
    Supported,
    Rejected,
    Unclear,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingAudit {
    pub request: String,
    pub contribution: String,
    pub auditor: PersonId,
    pub verdict: BindingVerdict,
    pub reason: String,
    pub evidence: String,
    pub inspected_sources: Vec<KnowledgeDeclarationRef>,
    pub evidence_complete: bool,
    pub at: DateTime<Utc>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum BindingEvent {
    Contributed {
        contribution: BindingContribution,
    },
    Challenged {
        challenge: BindingChallenge,
    },
    Audited {
        audit: BindingAudit,
    },
    Revoked {
        by: PersonId,
        reason: String,
        at: DateTime<Utc>,
    },
}
/// Composition-only result. No REST input can file an audit.
pub struct BindingInspection {
    pub verdict: BindingVerdict,
    pub reason: String,
    pub evidence: String,
    pub inspected_sources: Vec<KnowledgeDeclarationRef>,
    pub evidence_complete: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct PendingBindingAudit {
    pub board: BindingBoard,
    pub request: String,
    pub contribution: BindingContribution,
    pub challenge: Option<BindingChallenge>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PublicBindingAudit {
    pub request: String,
    pub contribution: String,
    pub verdict: BindingVerdict,
    pub evidence_complete: bool,
    pub at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize)]
pub struct BindingCredit {
    pub canonical: String,
    pub contribution: String,
    pub contributor: PersonId,
}
#[derive(Debug, Clone, Serialize)]
pub struct PublicBindingBoard {
    pub id: String,
    pub target: BindingTarget,
    pub context: String,
    pub license: String,
    pub authorized_by: PersonId,
    pub authorized_at: DateTime<Utc>,
    pub contributions: Vec<BindingContribution>,
    pub challenges: Vec<BindingChallenge>,
    pub audits: Vec<PublicBindingAudit>,
    pub credits: Vec<BindingCredit>,
}
#[derive(Debug, Clone, Serialize)]
pub struct PrivateBindingView {
    pub options: Vec<BindingOption>,
    pub boards: Vec<BindingBoard>,
}
impl BindingBoard {
    pub fn revoked(&self) -> bool {
        self.events
            .iter()
            .any(|e| matches!(e, BindingEvent::Revoked { .. }))
    }
    pub fn contributions(&self) -> Vec<&BindingContribution> {
        self.events
            .iter()
            .filter_map(|e| match e {
                BindingEvent::Contributed { contribution } => Some(contribution),
                _ => None,
            })
            .collect()
    }
    pub fn audits(&self) -> Vec<&BindingAudit> {
        self.events
            .iter()
            .filter_map(|e| match e {
                BindingEvent::Audited { audit } => Some(audit),
                _ => None,
            })
            .collect()
    }
    pub fn pending(&self) -> Option<PendingBindingAudit> {
        let audits = self.audits();
        for event in &self.events {
            let (request, contribution, challenge) = match event {
                BindingEvent::Contributed { contribution } => {
                    (&contribution.id, contribution, None)
                }
                BindingEvent::Challenged { challenge } => {
                    let contribution = self
                        .contributions()
                        .into_iter()
                        .find(|c| c.id == challenge.contribution)?;
                    (&challenge.id, contribution, Some(challenge.clone()))
                }
                _ => continue,
            };
            if !audits.iter().any(|a| a.request == *request) {
                return Some(PendingBindingAudit {
                    board: self.clone(),
                    request: request.clone(),
                    contribution: contribution.clone(),
                    challenge,
                });
            }
        }
        None
    }
    pub fn public(&self) -> PublicBindingBoard {
        let audits = self.audits();
        let mut credited = std::collections::BTreeSet::new();
        let mut credits = vec![];
        // First independently supported coverage receives credit. Rejected earlier
        // submissions cannot take that credit after a later successful appeal.
        let contributions = self.contributions();
        for first in audits
            .iter()
            .filter(|a| a.verdict == BindingVerdict::Supported)
        {
            let Some(contribution) = contributions.iter().find(|c| c.id == first.contribution)
            else {
                continue;
            };
            let effective = audits.iter().rev().find(|a| {
                contributions
                    .iter()
                    .any(|c| c.id == a.contribution && c.canonical == contribution.canonical)
            });
            if matches!(
                contribution.binding.relation,
                BindingRelation::Equivalent | BindingRelation::Instance | BindingRelation::Implies
            ) && effective.is_some_and(|a| a.verdict == BindingVerdict::Supported)
                && credited.insert(contribution.canonical.clone())
            {
                credits.push(BindingCredit {
                    canonical: contribution.canonical.clone(),
                    contribution: contribution.id.clone(),
                    contributor: contribution.contributor.clone(),
                });
            }
        }
        PublicBindingBoard {
            id: self.id.clone(),
            target: self.target.clone(),
            context: self.context.clone(),
            license: self.license.clone(),
            authorized_by: self.authorized_by.clone(),
            authorized_at: self.authorized_at,
            contributions: self.contributions().into_iter().cloned().collect(),
            challenges: self
                .events
                .iter()
                .filter_map(|e| match e {
                    BindingEvent::Challenged { challenge } => Some(challenge.clone()),
                    _ => None,
                })
                .collect(),
            audits: audits
                .into_iter()
                .map(|a| PublicBindingAudit {
                    request: a.request.clone(),
                    contribution: a.contribution.clone(),
                    verdict: a.verdict,
                    evidence_complete: a.evidence_complete,
                    at: a.at,
                })
                .collect(),
            credits,
        }
    }
}
impl NewBindingContribution {
    pub fn validate(&self, target: &BindingTarget) -> CoreResult<()> {
        binding_consent(&self.license, &self.consent)?;
        self.binding.validate(&target.subject)?;
        if self
            .binding
            .premises
            .iter()
            .any(|p| !target.sources.contains(p))
        {
            return Err(CoreError::invalid(
                "binding premises must be in the author-approved actual source results",
            ));
        }
        if serde_json::to_vec(self).map_or(true, |v| v.len() > 100_000) {
            return Err(CoreError::invalid(
                "binding contribution exceeds 100000 bytes",
            ));
        }
        let mut names = std::collections::BTreeSet::new();
        for p in &self.binding.parameters {
            if !names.insert(normalized(&p.parameter)) {
                return Err(CoreError::invalid("duplicate parameter binding"));
            }
        }
        Ok(())
    }
    pub fn canonical(&self, target: &BindingTarget) -> String {
        let mut premises = self
            .binding
            .premises
            .iter()
            .map(binding_digest)
            .collect::<Vec<_>>();
        premises.sort();
        let mut parameters = self
            .binding
            .parameters
            .iter()
            .map(|p| (normalized(&p.parameter), normalized(&p.expression)))
            .collect::<Vec<_>>();
        parameters.sort();
        let mut hypotheses = self
            .binding
            .hypotheses
            .iter()
            .map(|h| normalized(&h.hypothesis))
            .collect::<Vec<_>>();
        hypotheses.sort();
        hypotheses.dedup();
        binding_digest(&(
            (
                &target.subject.submission,
                target.subject.version,
                target.subject.claims_revision,
                &target.subject.claim,
                &target.subject.source_digest,
                &target.subject.claim_digest,
            ),
            premises,
            self.binding.relation,
            parameters,
            hypotheses,
        ))
    }
}
pub(crate) fn binding_consent(license: &str, consent: &str) -> CoreResult<()> {
    if license != BINDING_LICENSE || consent != BINDING_CONTRIBUTION_CONSENT {
        return Err(CoreError::invalid(
            "explicit CC-BY-4.0 contribution and AI inspection consent required",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindingAdmission {
    pub request: String,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BindingProfileEntry {
    pub submission: crate::ids::SubmissionId,
    pub board: String,
    pub statement: String,
    pub contribution: BindingContribution,
    pub credited: bool,
    pub latest_verdict: Option<BindingVerdict>,
}
