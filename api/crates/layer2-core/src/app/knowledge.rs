//! Durable observations share the review's revision-fenced history and GridFS
//! storage. Interfaces cannot write arbitrary model-supplied receipts.

use super::App;
use crate::{
    CoreError, CoreResult,
    ids::{ClaimId, SubmissionId},
    model::*,
};
use sha2::{Digest, Sha256};

impl App {
    pub async fn knowledge_subject(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        number: u32,
        claim: &ClaimId,
    ) -> CoreResult<CoverageSubject> {
        caller.require(Role::Reviewer)?;
        let submission = self.submission(caller, id).await?;
        if submission.involves(&caller.person) {
            return Err(CoreError::forbidden(
                "authors cannot file their own knowledge review",
            ));
        }
        let (file, _) = self.referee_file(id).await?;
        let round = file
            .current()
            .filter(|r| r.number == number)
            .ok_or_else(|| CoreError::conflict("knowledge work must name the current round"))?;
        if Some(round.version) != submission.current_version().map(|v| v.number)
            || round.claims_revision != submission.claims_revision
            || round.is_settled()
        {
            return Err(CoreError::conflict(
                "knowledge work refers to changed or settled review inputs",
            ));
        }
        let claim = submission
            .claims
            .iter()
            .find(|c| &c.id == claim)
            .ok_or_else(|| CoreError::not_found("statement", claim.as_str()))?;
        let subject = CoverageSubject {
            submission: id.clone(),
            version: round.version,
            claims_revision: round.claims_revision,
            round: number,
            claim: claim.id.clone(),
            source_digest: format!("sha256:{}", round.source_sha256),
            claim_digest: format!(
                "sha256:{:x}",
                Sha256::digest(
                    serde_json::to_vec(claim)
                        .map_err(|_| CoreError::invalid("claim cannot be encoded"))?
                )
            ),
        };
        subject.validate()?;
        Ok(subject)
    }

    /// Worker-only service operation. The caller must retain its lease fence;
    /// the aggregate revision additionally rejects concurrent writes.
    pub async fn record_knowledge_event(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        event: KnowledgeEvent,
    ) -> CoreResult<RefereeFile> {
        caller.require(Role::Reviewer)?;
        let (mut file, stored) = self.referee_file(id).await?;
        // Identical replay is read-only even after a round has settled.
        if file.knowledge_events.contains(&event) {
            return Ok(file);
        }
        let subject = event.subject();
        let current = self
            .knowledge_subject(caller, id, subject.round, &subject.claim)
            .await?;
        if &current != subject {
            return Err(CoreError::conflict("knowledge subject changed"));
        }
        if file.knowledge_events.len() >= 10_000 {
            return Err(CoreError::conflict(
                "review knowledge history reached its event limit",
            ));
        }
        match &event {
            KnowledgeEvent::QueryAdmitted { intent } => {
                intent.validate()?;
                if file.knowledge_events.iter().any(|e| {
                    matches!(e,
                    KnowledgeEvent::QueryAdmitted { intent: previous } if previous.id == intent.id)
                }) {
                    return Err(CoreError::conflict(
                        "query id already admitted with different inputs",
                    ));
                }
                let pinned = file
                    .knowledge_events
                    .iter()
                    .filter_map(|event| match event {
                        KnowledgeEvent::QueryAdmitted { intent }
                            if intent.subject.round == subject.round =>
                        {
                            intent.snapshot.as_ref().zip(intent.index_revision.as_ref())
                        }
                        _ => None,
                    })
                    .next();
                if let Some((snapshot, index)) = pinned
                    && (intent.snapshot.as_ref() != Some(snapshot)
                        || intent.index_revision.as_ref() != Some(index))
                {
                    return Err(CoreError::conflict(
                        "review knowledge snapshot is already pinned",
                    ));
                }
            }
            KnowledgeEvent::QueryCompleted { receipt } => {
                receipt.validate()?;
                if let Some(response) = &receipt.response {
                    let bytes = self
                        .ports
                        .blobs
                        .get(&response.id)
                        .await?
                        .ok_or_else(|| CoreError::invalid("query response was not stored"))?;
                    if bytes.len() as u64 != response.bytes
                        || format!("{:x}", Sha256::digest(&bytes)) != response.sha256
                    {
                        return Err(CoreError::invalid("query response checksum mismatch"));
                    }
                }
                if !file.knowledge_events.iter().any(|e| {
                    matches!(e,
                    KnowledgeEvent::QueryAdmitted { intent } if intent.matches(receipt))
                }) {
                    return Err(CoreError::conflict(
                        "query result has no matching durable admission",
                    ));
                }
                if file.knowledge_events.iter().any(|e| matches!(e,
                    KnowledgeEvent::QueryCompleted { receipt: previous } if previous.id == receipt.id)) {
                    return Err(CoreError::conflict("completed query receipt is immutable"));
                }
            }
            KnowledgeEvent::CoverageAssessed { assessment, .. } => {
                let receipts = file
                    .knowledge_events
                    .iter()
                    .filter_map(|e| match e {
                        KnowledgeEvent::QueryCompleted { receipt } => Some(receipt.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assessment.validate(&receipts)?;
                if let CoverageQualification::SourceAudited { auditor, .. } =
                    &assessment.qualification
                    && auditor != &caller.person
                {
                    return Err(CoreError::forbidden(
                        "source audit must identify its actual reviewer",
                    ));
                }
            }
        }
        file.knowledge_events.push(event);
        self.store_referee_file(&mut file, stored).await?;
        Ok(file)
    }
}
