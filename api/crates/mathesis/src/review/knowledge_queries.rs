//! Bounded semantic query proposals become durable system work, never receipts.
use super::worker::{Failure, Worker};
use mathesis_core::{ids::ClaimId, model::*, ports::LeasedJob};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Deserialize, Serialize)]
struct Proposal {
    claim: ClaimId,
    definitions: Vec<String>,
    hypotheses: Vec<String>,
    quantifiers: String,
    conclusion: String,
    search_terms: Vec<String>,
    reason: String,
}

impl Proposal {
    fn query(&self) -> Option<String> {
        let text = |s: &str, limit| !s.trim().is_empty() && s.len() <= limit;
        if self.definitions.len() > 30
            || self.hypotheses.len() > 30
            || self.search_terms.is_empty()
            || self.search_terms.len() > 20
            || !self
                .definitions
                .iter()
                .chain(&self.hypotheses)
                .chain(&self.search_terms)
                .all(|s| text(s, 2_000))
            || !text(&self.quantifiers, 6_000)
            || !text(&self.conclusion, 6_000)
            || !text(&self.reason, 2_000)
        {
            return None;
        }
        // The actual search terms preserve the extracted mathematical inputs;
        // the reason stays in the digest-bound scope, not in the ranking query.
        let query = format!(
            "Definitions: {}\nHypotheses: {}\nQuantifiers: {}\nConclusion: {}\nRelated mathematical terms: {}",
            self.definitions.join("; "),
            self.hypotheses.join("; "),
            self.quantifiers,
            self.conclusion,
            self.search_terms.join("; ")
        );
        (query.len() <= 30_000).then_some(query)
    }
}

#[derive(Default)]
pub(super) struct QueryProposalOutcome {
    pub admitted: bool,
    pub unexecuted: BTreeSet<ClaimId>,
    pub invalid: usize,
}

impl Worker {
    pub(super) async fn admit_query_proposals(
        &self,
        job: &LeasedJob,
        paper: &Submission,
        number: u32,
        raw: &[serde_json::Value],
        origin: &str,
    ) -> Result<QueryProposalOutcome, Failure> {
        let view = self.app.referee(&self.referee_account, &paper.id).await?;
        let round = view
            .rounds
            .iter()
            .find(|r| r.number == number)
            .ok_or_else(|| Failure::Permanent("query proposal round disappeared".into()))?;
        let mut outcome = QueryProposalOutcome::default();
        if round.knowledge_contract < 2 || raw.is_empty() {
            return Ok(outcome);
        }
        let intents = round
            .knowledge_events
            .iter()
            .filter_map(|event| match event {
                KnowledgeEvent::QueryAdmitted { intent } => Some(intent),
                _ => None,
            })
            .collect::<Vec<_>>();
        let pin = intents.first().ok_or_else(|| {
            Failure::Permanent("semantic queries require the original source pin".into())
        })?;
        let used = intents.iter().any(|i| i.scopes.iter().any(|s| s == origin));
        let mut plans = vec![];
        let mut seen = BTreeSet::new();
        // Oversized model plans cannot silently execute an arbitrary prefix.
        if raw.len() > 400 {
            outcome.invalid = raw.len();
            outcome
                .unexecuted
                .extend(paper.claims.iter().map(|c| c.id.clone()));
            return Ok(outcome);
        }
        for value in raw {
            let proposal: Proposal = match serde_json::from_value(value.clone()) {
                Ok(proposal) => proposal,
                Err(_) => {
                    outcome.invalid += 1;
                    if let Some(id) = value["claim"].as_str().and_then(|id| {
                        paper
                            .claims
                            .iter()
                            .find(|c| c.id.as_str() == id)
                            .map(|c| c.id.clone())
                    }) {
                        outcome.unexecuted.insert(id);
                    } else {
                        // Without a valid claim id the evidence gap cannot be
                        // assigned safely to only one comparison.
                        outcome
                            .unexecuted
                            .extend(paper.claims.iter().map(|c| c.id.clone()));
                    }
                    continue;
                }
            };
            if !paper.claims.iter().any(|c| c.id == proposal.claim) {
                outcome.invalid += 1;
                outcome
                    .unexecuted
                    .extend(paper.claims.iter().map(|c| c.id.clone()));
                continue;
            }
            if !seen.insert(proposal.claim.clone()) {
                outcome.invalid += 1;
                outcome.unexecuted.insert(proposal.claim);
                continue;
            }
            let Some(full_query) = proposal.query() else {
                outcome.invalid += 1;
                outcome.unexecuted.insert(proposal.claim);
                continue;
            };
            let query = if round.knowledge_contract >= 3 {
                proposal.search_terms.join("; ")
            } else {
                full_query.clone()
            };
            let mut scopes = vec![
                super::knowledge::QUERY_SCOPE.into(),
                origin.into(),
                format!("Retrieval gap: {}", proposal.reason),
            ];
            if round.knowledge_contract >= 3 {
                use sha2::{Digest, Sha256};
                scopes.push(KNOWLEDGE_RANKER_SCOPE.into());
                let structured = serde_json::to_vec(&proposal).map_err(|_| {
                    Failure::Permanent("cannot encode mathematical reformulation".into())
                })?;
                scopes.push(format!("Standard search terms from mathematical reformulation sha256:{:x}; full definitions, hypotheses, quantifiers and conclusion retained in the originating model response.", Sha256::digest(structured)));
            }
            if used {
                if !intents.iter().any(|i| {
                    i.subject.claim == proposal.claim
                        && i.query == query
                        && i.scopes.iter().any(|s| s == origin)
                        && (round.knowledge_contract < 3 || i.scopes == scopes)
                }) {
                    outcome.unexecuted.insert(proposal.claim);
                }
                continue;
            }
            let subject = self
                .app
                .knowledge_subject(&self.referee_account, &paper.id, number, &proposal.claim)
                .await?;
            let request_digest = KnowledgeQueryReceipt::request_digest(
                &subject,
                &pin.snapshot,
                &pin.index_revision,
                &query,
                &scopes,
            );
            plans.push(KnowledgeQueryIntent {
                id: format!(
                    "knowledge:r{number}:{}:{}",
                    proposal.claim,
                    &request_digest[7..]
                ),
                subject,
                snapshot: pin.snapshot.clone(),
                index_revision: pin.index_revision.clone(),
                request_digest,
                query,
                scopes,
                admitted_at: chrono::Utc::now(),
            });
        }
        plans.retain(|intent| !outcome.unexecuted.contains(&intent.subject.claim));
        if outcome.invalid > 0 || !outcome.unexecuted.is_empty() {
            outcome
                .unexecuted
                .extend(plans.iter().map(|intent| intent.subject.claim.clone()));
            return Ok(outcome);
        }
        if !plans.is_empty() {
            self.ensure_lease(job).await?;
            self.app
                .admit_knowledge_plan_leased(&self.referee_account, job, self.jobs.as_ref(), plans)
                .await?;
            outcome.admitted = true;
        }
        Ok(outcome)
    }

    /// Resume every admitted semantic query before allowing a new audit prompt.
    pub(super) async fn resume_query_plan(
        &self,
        job: &LeasedJob,
        paper: &Submission,
        number: u32,
    ) -> Result<bool, Failure> {
        let view = self.app.referee(&self.referee_account, &paper.id).await?;
        let round = view
            .rounds
            .iter()
            .find(|r| r.number == number)
            .ok_or_else(|| Failure::Permanent("query plan round disappeared".into()))?;
        let done: BTreeSet<_> = round
            .knowledge_events
            .iter()
            .filter_map(|event| match event {
                KnowledgeEvent::QueryCompleted { receipt } => Some(receipt.id.as_str()),
                _ => None,
            })
            .collect();
        let mut resumed = false;
        for event in &round.knowledge_events {
            if let KnowledgeEvent::QueryAdmitted { intent } = event
                && !done.contains(intent.id.as_str())
            {
                self.ensure_lease(job).await?;
                self.execute_knowledge_query(
                    job,
                    intent.clone(),
                    false,
                    KnowledgeQueryState::Unavailable,
                )
                .await?;
                resumed = true;
            }
        }
        Ok(resumed)
    }
}
