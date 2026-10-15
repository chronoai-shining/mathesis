//! Durable observations share the review's revision-fenced history and GridFS
//! storage. Interfaces cannot write arbitrary model-supplied receipts.

use super::App;
use crate::{
    CoreError, CoreResult,
    ids::{ClaimId, SubmissionId},
    model::*,
};
use sha2::{Digest, Sha256};

pub(super) fn knowledge_admission(
    file: &RefereeFile,
    round: &RefereeRound,
    reading: &AuditedClaim,
) -> KnowledgeAdmission {
    let receipts = file
        .knowledge_events
        .iter()
        .filter_map(|event| match event {
            KnowledgeEvent::QueryCompleted { receipt }
                if receipt.subject.submission == file.id
                    && receipt.subject.round == round.number
                    && receipt.subject.version == round.version
                    && receipt.subject.claims_revision == round.claims_revision
                    && receipt.subject.claim == reading.claim =>
            {
                Some(receipt.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let audited_relation = reading
        .coverage
        .as_ref()
        .filter(|c| {
            c.validate(&receipts).is_ok()
                && matches!(c.qualification, CoverageQualification::SourceAudited { .. })
                && c.receipt_ids.len() == receipts.len()
                && receipts.iter().all(|r| c.receipt_ids.contains(&r.id))
        })
        .map(|c| c.relation);
    let admitted = file
        .knowledge_events
        .iter()
        .filter(|event| {
            matches!(event,
        KnowledgeEvent::QueryAdmitted { intent } if intent.subject.round == round.number
            && intent.subject.claim == reading.claim)
        })
        .count();
    KnowledgeAdmission {
        receipt_ids: receipts.iter().map(|r| r.id.clone()).collect(),
        complete: !receipts.is_empty()
            && receipts.len() == admitted
            && (round.knowledge_contract < 2
                || receipts
                    .iter()
                    .any(|r| r.scopes.iter().any(|s| s == KNOWLEDGE_ORACLE_SCOPE)))
            && receipts
                .iter()
                .all(|r| r.state == KnowledgeQueryState::Complete),
        audited_relation,
    }
}

impl App {
    pub fn knowledge_capabilities(&self) -> KnowledgeCapabilities {
        KnowledgeCapabilities::current()
    }

    /// Admit a complete model-proposed query plan in one revision-fenced write.
    /// A crash cannot preserve only the first query and silently lose the rest.
    pub async fn admit_knowledge_plan_leased(
        &self,
        caller: &Caller,
        job: &crate::ports::LeasedJob,
        leases: &dyn crate::ports::JobLease,
        intents: Vec<KnowledgeQueryIntent>,
    ) -> CoreResult<RefereeFile> {
        caller.require(Role::Reviewer)?;
        if intents.is_empty() || intents.len() > 400 {
            return Err(CoreError::invalid(
                "knowledge plan needs 1–400 bounded queries",
            ));
        }
        let (mut file, stored) = self.referee_file(&job.submission).await?;
        let mut ids = std::collections::BTreeSet::new();
        let mut claims = std::collections::BTreeSet::new();
        let origin = |intent: &KnowledgeQueryIntent| {
            let oracle = intent.scopes.iter().any(|s| s == KNOWLEDGE_ORACLE_SCOPE);
            let cma = intent.scopes.iter().any(|s| s == KNOWLEDGE_CMA_SCOPE);
            match (oracle, cma) {
                (true, false) => Some(KNOWLEDGE_ORACLE_SCOPE),
                (false, true) => Some(KNOWLEDGE_CMA_SCOPE),
                _ => None,
            }
        };
        let plan_origin = origin(&intents[0])
            .ok_or_else(|| CoreError::invalid("knowledge plan needs one recognized origin"))?;
        if file.current().is_none_or(|r| r.knowledge_contract < 2) {
            return Err(CoreError::conflict(
                "this round has no semantic query contract",
            ));
        }
        let mut replayed = 0;
        for intent in &intents {
            intent.validate()?;
            validate_knowledge_ranker(file.current().unwrap().knowledge_contract, &intent.scopes)?;
            if !ids.insert(&intent.id)
                || !claims.insert(&intent.subject.claim)
                || intent.subject.round != intents[0].subject.round
                || origin(intent) != Some(plan_origin)
            {
                return Err(CoreError::invalid(
                    "knowledge plan has duplicate claims/ids or mixed rounds/origins",
                ));
            }
            let subject = self
                .knowledge_subject(
                    caller,
                    &job.submission,
                    intent.subject.round,
                    &intent.subject.claim,
                )
                .await?;
            if intent.subject != subject {
                return Err(CoreError::conflict("knowledge plan subject changed"));
            }
            let pin = file
                .knowledge_events
                .iter()
                .find_map(|event| match event {
                    KnowledgeEvent::QueryAdmitted { intent: old }
                        if old.subject.round == intent.subject.round =>
                    {
                        Some((&old.snapshot, &old.index_revision))
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    CoreError::conflict("additional queries require the original round pin")
                })?;
            if pin != (&intent.snapshot, &intent.index_revision) {
                return Err(CoreError::conflict(
                    "knowledge plan changed its pinned source",
                ));
            }
            if let Some(old) = file.knowledge_events.iter().find_map(|event| match event {
                KnowledgeEvent::QueryAdmitted { intent: old } if old.id == intent.id => Some(old),
                _ => None,
            }) {
                if old != intent {
                    return Err(CoreError::conflict(
                        "query id already admitted with different inputs",
                    ));
                }
                replayed += 1;
            }
        }
        if replayed == intents.len() {
            return Ok(file);
        }
        if replayed > 0 {
            return Err(CoreError::conflict(
                "a knowledge plan cannot overwrite an earlier partial plan",
            ));
        }
        if file.knowledge_events.iter().any(|event| {
            matches!(event,
            KnowledgeEvent::QueryAdmitted { intent }
                if intent.subject.round == intents[0].subject.round
                    && origin(intent) == Some(plan_origin))
        }) {
            return Err(CoreError::conflict(
                "this query origin has already used its round budget",
            ));
        }
        if file.knowledge_events.len() + intents.len() > 10_000 {
            return Err(CoreError::conflict(
                "review knowledge history reached its event limit",
            ));
        }
        file.knowledge_events.extend(
            intents
                .into_iter()
                .map(|intent| KnowledgeEvent::QueryAdmitted { intent }),
        );
        if !leases.renew(job).await? {
            return Err(CoreError::conflict("knowledge plan lost its lease"));
        }
        self.store_referee_file(&mut file, stored).await?;
        Ok(file)
    }

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
            || round.audit.is_settled()
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
        self.record_knowledge_event_inner(caller, id, event, None)
            .await
    }

    pub async fn record_knowledge_event_leased(
        &self,
        caller: &Caller,
        job: &crate::ports::LeasedJob,
        leases: &dyn crate::ports::JobLease,
        event: KnowledgeEvent,
    ) -> CoreResult<RefereeFile> {
        self.record_knowledge_event_inner(caller, &job.submission, event, Some((job, leases)))
            .await
    }

    /// Store the actual bounded adapter response with the same blob authority as
    /// the review, then append its completion under the worker's live lease.
    pub async fn complete_knowledge_query_leased(
        &self,
        caller: &Caller,
        job: &crate::ports::LeasedJob,
        leases: &dyn crate::ports::JobLease,
        mut receipt: KnowledgeQueryReceipt,
        response: Option<&[u8]>,
    ) -> CoreResult<RefereeFile> {
        caller.require(Role::Reviewer)?;
        let subject = self
            .knowledge_subject(
                caller,
                &job.submission,
                receipt.subject.round,
                &receipt.subject.claim,
            )
            .await?;
        if subject != receipt.subject || !leases.renew(job).await? {
            return Err(CoreError::conflict(
                "knowledge query lost its input or lease fence",
            ));
        }
        let (file, _) = self.referee_file(&job.submission).await?;
        if let Some(prior) = file.knowledge_events.iter().find_map(|event| match event {
            KnowledgeEvent::QueryCompleted { receipt: prior } if prior.id == receipt.id => {
                Some(prior)
            }
            _ => None,
        }) {
            receipt.response = prior.response.clone();
            let bytes_match = match (&prior.response, response) {
                (Some(blob), Some(bytes)) => {
                    blob.bytes == bytes.len() as u64
                        && blob.sha256 == format!("{:x}", Sha256::digest(bytes))
                }
                (None, None) => true,
                _ => false,
            };
            if &receipt == prior && bytes_match {
                return Ok(file);
            }
            return Err(CoreError::conflict("completed query receipt is immutable"));
        }
        if let Some(bytes) = response {
            if bytes.is_empty() || bytes.len() > 2 * 1024 * 1024 {
                return Err(CoreError::invalid(
                    "knowledge response exceeds its byte limit",
                ));
            }
            receipt.response = Some(self.ports.blobs.put(bytes, "application/json").await?);
        } else {
            receipt.response = None;
        }
        self.record_knowledge_event_leased(
            caller,
            job,
            leases,
            KnowledgeEvent::QueryCompleted { receipt },
        )
        .await
    }

    pub async fn knowledge_response(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        receipt_id: &str,
    ) -> CoreResult<Option<Vec<u8>>> {
        // The ordinary referee projection enforces current author/staff access.
        let view = self.referee(caller, id).await?;
        let blob = view
            .rounds
            .iter()
            .flat_map(|r| &r.knowledge_events)
            .find_map(|e| match e {
                KnowledgeEvent::QueryCompleted { receipt } if receipt.id == receipt_id => {
                    receipt.response.as_ref()
                }
                _ => None,
            });
        let Some(blob) = blob else {
            return Ok(None);
        };
        let bytes = self
            .ports
            .blobs
            .get(&blob.id)
            .await?
            .ok_or_else(|| CoreError::Unavailable("stored query response is missing".into()))?;
        if bytes.len() as u64 != blob.bytes
            || format!("{:x}", Sha256::digest(&bytes)) != blob.sha256
        {
            return Err(CoreError::Unavailable(
                "stored query response checksum mismatch".into(),
            ));
        }
        Ok(Some(bytes))
    }

    async fn record_knowledge_event_inner(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        event: KnowledgeEvent,
        lease: Option<(&crate::ports::LeasedJob, &dyn crate::ports::JobLease)>,
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
                validate_knowledge_ranker(
                    file.current().unwrap().knowledge_contract,
                    &intent.scopes,
                )?;
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
                            Some((&intent.snapshot, &intent.index_revision))
                        }
                        _ => None,
                    })
                    .next();
                if let Some((snapshot, index)) = pinned
                    && (&intent.snapshot != snapshot || &intent.index_revision != index)
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
        if let Some((job, leases)) = lease
            && !leases.renew(job).await?
        {
            return Err(CoreError::conflict("knowledge query lost its lease"));
        }
        self.store_referee_file(&mut file, stored).await?;
        Ok(file)
    }
}
