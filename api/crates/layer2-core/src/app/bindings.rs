use super::App;
use crate::{
    CoreError, CoreResult,
    ids::SubmissionId,
    model::*,
    ports::{JobKind, JobLease, LeasedJob, Listing},
};

fn shareable(paper: &Submission) -> bool {
    paper.purpose == SubmissionPurpose::Publication
        && matches!(paper.status, SubmissionStatus::Accepted { .. })
        && paper.analysis_visibility == Visibility::Public
        && paper.open_to_contributors
}
fn options(paper: &Submission, file: &RefereeFile) -> Vec<BindingOption> {
    if !shareable(paper) {
        return vec![];
    }
    let Some(round) = file.rounds.iter().rev().find(|r| {
        Some(r.version) == paper.current_version().map(|v| v.number)
            && r.claims_revision == paper.claims_revision
            && r.audit.done().is_some()
    }) else {
        return vec![];
    };
    paper
        .claims
        .iter()
        .filter_map(|claim| {
            let subject = CoverageSubject {
                submission: paper.id.clone(),
                version: round.version,
                claims_revision: paper.claims_revision,
                round: round.number,
                claim: claim.id.clone(),
                source_digest: format!("sha256:{}", paper.current_version()?.archive.sha256),
                claim_digest: binding_digest(claim),
            };
            if !super::research::subject_current(paper, &subject) {
                return None;
            }
            let mut sources = vec![];
            for receipt in receipts(file, &subject) {
                for source in &receipt.returned {
                    if !sources.contains(source) {
                        sources.push(source.clone());
                    }
                }
            }
            if sources.is_empty() {
                return None;
            }
            sources.sort_by(|a, b| {
                (&a.snapshot, &a.statement_id).cmp(&(&b.snapshot, &b.statement_id))
            });
            let target = BindingTarget {
                subject,
                statement: claim.statement.clone(),
                sources,
            };
            Some(BindingOption {
                digest: binding_digest(&target),
                target,
            })
        })
        .collect()
}
fn receipts<'a>(
    file: &'a RefereeFile,
    subject: &CoverageSubject,
) -> Vec<&'a KnowledgeQueryReceipt> {
    file.knowledge_events
        .iter()
        .filter_map(|e| match e {
            KnowledgeEvent::QueryCompleted { receipt }
                if receipt.subject == *subject
                    && receipt.response.is_some()
                    && receipt.validate().is_ok() =>
            {
                Some(receipt)
            }
            _ => None,
        })
        .collect()
}
fn active(paper: &Submission, board: &BindingBoard) -> bool {
    shareable(paper)
        && !board.revoked()
        && board.consent == BINDING_SHARING_CONSENT
        && super::research::subject_current(paper, &board.target.subject)
        && paper
            .processing_consent
            .as_ref()
            .is_some_and(|c| c.version == AI_PROCESSING_CONSENT_VERSION)
}
fn board<'a>(file: &'a RefereeFile, key: &str) -> CoreResult<&'a BindingBoard> {
    file.binding_boards
        .iter()
        .find(|b| b.id == key)
        .ok_or_else(|| CoreError::not_found("binding summary", key))
}
fn rate(
    board: &BindingBoard,
    person: &crate::ids::PersonId,
    now: chrono::DateTime<chrono::Utc>,
) -> CoreResult<()> {
    if board.events.len() >= 120 {
        return Err(CoreError::conflict("binding discussion history is full"));
    }
    let recent = board
        .events
        .iter()
        .filter(|e| match e {
            BindingEvent::Contributed { contribution: c } => {
                c.contributor == *person && c.at > now - chrono::Duration::days(1)
            }
            BindingEvent::Challenged { challenge: c } => {
                c.challenger == *person && c.at > now - chrono::Duration::days(1)
            }
            _ => false,
        })
        .count();
    if recent >= 5 {
        return Err(CoreError::conflict(
            "at most five contributions or challenges per person per summary in 24 hours",
        ));
    }
    Ok(())
}
impl App {
    pub async fn binding_profile(
        &self,
        person: &crate::ids::PersonId,
        before: Option<String>,
    ) -> CoreResult<Listing<BindingProfileEntry>> {
        if before.as_ref().is_some_and(|b| b.len() > 250) || person.as_str().len() > 250 {
            return Err(CoreError::invalid(
                "invalid binding profile cursor or person",
            ));
        }
        let page = self.ports.referees.binding_sources(25, before).await?;
        let mut items = vec![];
        for id in page.items {
            let Some(paper) = self.ports.submissions.get(&id).await? else {
                continue;
            };
            let Some(file) = self.ports.referees.get(&id).await? else {
                continue;
            };
            for b in file.binding_boards.iter().filter(|b| active(&paper, b)) {
                let public = b.public();
                for c in public
                    .contributions
                    .iter()
                    .filter(|c| c.contributor == *person)
                {
                    let latest_verdict = public
                        .audits
                        .iter()
                        .rev()
                        .find(|a| {
                            public.contributions.iter().any(|other| {
                                other.id == a.contribution && other.canonical == c.canonical
                            })
                        })
                        .map(|a| a.verdict);
                    items.push(BindingProfileEntry {
                        submission: id.clone(),
                        board: b.id.clone(),
                        statement: b.target.statement.chars().take(400).collect(),
                        contribution: c.clone(),
                        credited: public
                            .credits
                            .iter()
                            .any(|credit| credit.contribution == c.id),
                        latest_verdict,
                    });
                }
            }
        }
        Ok(Listing {
            items,
            next_before: page.next_before,
        })
    }

    pub async fn private_bindings(
        &self,
        caller: &Caller,
        id: &SubmissionId,
    ) -> CoreResult<PrivateBindingView> {
        let paper = self.submission(caller, id).await?;
        let (file, _) = self.referee_file(id).await?;
        Ok(PrivateBindingView {
            options: options(&paper, &file),
            boards: file.binding_boards,
        })
    }
    pub async fn publish_binding_board(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        new: NewBindingBoard,
    ) -> CoreResult<PublicBindingBoard> {
        require_text("public mathematical context", &new.context, 20_000)?;
        if new.license != BINDING_LICENSE
            || new.consent != BINDING_SHARING_CONSENT
            || new.sources.is_empty()
            || new.sources.len() > 40
        {
            return Err(CoreError::invalid(
                "select 1–40 actual source declarations and explicitly license the exact summary for public sharing",
            ));
        }
        let key = binding_digest(&(id, &caller.person, &new))
            .trim_start_matches("sha256:")
            .to_owned();
        loop {
            let paper = self.submission(caller, id).await?;
            Self::require_submitter(caller, &paper)?;
            let (mut file, stored) = self.referee_file(id).await?;
            if let Some(b) = file.binding_boards.iter().find(|b| b.id == key) {
                if active(&paper, b) {
                    return Ok(b.public());
                }
                return Err(CoreError::conflict(
                    "the previously authorized summary is no longer active",
                ));
            }
            let option = options(&paper, &file)
                .into_iter()
                .find(|o| o.digest == new.option_digest)
                .ok_or_else(|| {
                    CoreError::conflict("the selected exact statement or source results changed")
                })?;
            if new
                .sources
                .iter()
                .any(|s| !option.target.sources.contains(s))
                || new
                    .sources
                    .iter()
                    .map(binding_digest)
                    .collect::<std::collections::BTreeSet<_>>()
                    .len()
                    != new.sources.len()
            {
                return Err(CoreError::invalid(
                    "select distinct declarations from the displayed actual source results",
                ));
            }
            if file.binding_boards.len() >= 10
                || file.binding_boards.iter().any(|b| {
                    b.target.subject.submission == option.target.subject.submission
                        && b.target.subject.version == option.target.subject.version
                        && b.target.subject.claims_revision == option.target.subject.claims_revision
                        && b.target.subject.claim == option.target.subject.claim
                        && active(&paper, b)
                })
            {
                return Err(CoreError::conflict(
                    "the target already has an active summary or the summary history is full",
                ));
            }
            let ids = receipts(&file, &option.target.subject)
                .iter()
                .filter(|r| new.sources.iter().any(|s| r.returned.contains(s)))
                .map(|r| r.id.clone())
                .collect::<Vec<_>>();
            let snapshots = new
                .sources
                .iter()
                .map(|s| &s.snapshot)
                .collect::<std::collections::BTreeSet<_>>();
            if snapshots.len() != 1 || ids.len() > 40 {
                return Err(CoreError::invalid(
                    "one summary must use one bounded publication snapshot",
                ));
            }
            let mut target = option.target;
            target.sources = new.sources.clone();
            let b = BindingBoard {
                id: key.clone(),
                target,
                context: new.context.clone(),
                license: new.license.clone(),
                consent: new.consent.clone(),
                authorized_by: caller.person.clone(),
                authorized_at: self.ports.clock.now(),
                receipt_ids: ids,
                events: vec![],
            };
            if !active(&paper, &b) {
                return Err(CoreError::conflict(
                    "current source and AI processing consent are required",
                ));
            }
            let public = b.public();
            file.binding_boards.push(b);
            match self.store_referee_file(&mut file, stored).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => {
                    result?;
                    return Ok(public);
                }
            }
        }
    }
    pub async fn revoke_binding_board(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        key: &str,
        reason: String,
    ) -> CoreResult<()> {
        require_text("revocation reason", &reason, 2000)?;
        loop {
            let paper = self.submission(caller, id).await?;
            Self::require_submitter(caller, &paper)?;
            let (mut file, stored) = self.referee_file(id).await?;
            if board(&file, key)?.revoked() {
                return Ok(());
            }
            file.binding_boards
                .iter_mut()
                .find(|b| b.id == key)
                .unwrap()
                .events
                .push(BindingEvent::Revoked {
                    by: caller.person.clone(),
                    reason: reason.clone(),
                    at: self.ports.clock.now(),
                });
            match self.store_referee_file(&mut file, stored).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => {
                    result?;
                    break;
                }
            }
        }
        self.ports.queue.enqueue(id, JobKind::BindingAudit).await
    }
    pub async fn public_binding_board(
        &self,
        id: &SubmissionId,
        key: &str,
    ) -> CoreResult<PublicBindingBoard> {
        let paper = self.load(id).await?;
        let (file, _) = self.referee_file(id).await?;
        let b = board(&file, key)?;
        if !active(&paper, b) {
            return Err(CoreError::not_found("binding summary", key));
        }
        Ok(b.public())
    }
    pub async fn binding_directory(
        &self,
        limit: Option<u32>,
        before: Option<String>,
    ) -> CoreResult<Listing<serde_json::Value>> {
        if before.as_ref().is_some_and(|b| b.len() > 250) {
            return Err(CoreError::invalid("invalid binding directory cursor"));
        }
        let page = self
            .ports
            .referees
            .binding_sources(super::clamp(limit).min(25), before)
            .await?;
        let mut items = vec![];
        for id in page.items {
            let Some(paper) = self.ports.submissions.get(&id).await? else {
                continue;
            };
            let Some(file) = self.ports.referees.get(&id).await? else {
                continue;
            };
            for b in file.binding_boards.iter().filter(|b| active(&paper, b)) {
                items.push(serde_json::json!({"id":b.id,"submission":id,"claim":b.target.subject.claim,"statement":b.target.statement.chars().take(400).collect::<String>(),"authorized_by":b.authorized_by,"authorized_at":b.authorized_at,"contributions":b.contributions().len(),"credits":b.public().credits.len()}));
            }
        }
        Ok(Listing {
            items,
            next_before: page.next_before,
        })
    }
    pub async fn contribute_binding(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        key: &str,
        new: NewBindingContribution,
    ) -> CoreResult<PublicBindingBoard> {
        let request = binding_digest(&(key, &caller.person, &new))
            .trim_start_matches("sha256:")
            .to_owned();
        loop {
            let paper = self.load(id).await?;
            if paper.involves(&caller.person) {
                return Err(CoreError::forbidden(
                    "authors cannot claim independent binding credit on their own submission",
                ));
            }
            let (mut file, stored) = self.referee_file(id).await?;
            let b = board(&file, key)?;
            if !active(&paper, b) {
                return Err(CoreError::not_found("binding summary", key));
            }
            new.validate(&b.target)?;
            if b.contributions().iter().any(|c| c.id == request) {
                break;
            }
            let now = self.ports.clock.now();
            rate(b, &caller.person, now)?;
            self.ports
                .referees
                .admit_binding_submission(&caller.person, &request, now)
                .await?;
            let contribution = BindingContribution {
                id: request.clone(),
                canonical: new.canonical(&b.target),
                contributor: caller.person.clone(),
                binding: new.binding.clone(),
                license: new.license.clone(),
                consent: new.consent.clone(),
                at: now,
            };
            file.binding_boards
                .iter_mut()
                .find(|b| b.id == key)
                .unwrap()
                .events
                .push(BindingEvent::Contributed { contribution });
            match self.store_referee_file(&mut file, stored).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => {
                    result?;
                    break;
                }
            }
        }
        self.ports.queue.enqueue(id, JobKind::BindingAudit).await?;
        self.public_binding_board(id, key).await
    }
    pub async fn challenge_binding(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        key: &str,
        new: NewBindingChallenge,
    ) -> CoreResult<PublicBindingBoard> {
        binding_consent(&new.license, &new.consent)?;
        require_text("challenge or appeal evidence", &new.evidence, 20_000)?;
        let request = binding_digest(&(key, &caller.person, &new))
            .trim_start_matches("sha256:")
            .to_owned();
        loop {
            let paper = self.load(id).await?;
            let (mut file, stored) = self.referee_file(id).await?;
            let b = board(&file, key)?;
            if !active(&paper, b) {
                return Err(CoreError::not_found("binding summary", key));
            }
            if !b.contributions().iter().any(|c| c.id == new.contribution) {
                return Err(CoreError::not_found(
                    "binding contribution",
                    &new.contribution,
                ));
            }
            if b.events
                .iter()
                .any(|e| matches!(e,BindingEvent::Challenged{challenge} if challenge.id==request))
            {
                break;
            }
            let now = self.ports.clock.now();
            rate(b, &caller.person, now)?;
            self.ports
                .referees
                .admit_binding_submission(&caller.person, &request, now)
                .await?;
            let challenge = BindingChallenge {
                id: request.clone(),
                contribution: new.contribution.clone(),
                challenger: caller.person.clone(),
                evidence: new.evidence.clone(),
                license: new.license.clone(),
                consent: new.consent.clone(),
                at: now,
            };
            file.binding_boards
                .iter_mut()
                .find(|b| b.id == key)
                .unwrap()
                .events
                .push(BindingEvent::Challenged { challenge });
            match self.store_referee_file(&mut file, stored).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => {
                    result?;
                    break;
                }
            }
        }
        self.ports.queue.enqueue(id, JobKind::BindingAudit).await?;
        self.public_binding_board(id, key).await
    }
    pub async fn binding_active(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        key: &str,
    ) -> CoreResult<bool> {
        caller.require(Role::Reviewer)?;
        let paper = self.load(id).await?;
        let (file, _) = self.referee_file(id).await?;
        Ok(file
            .binding_boards
            .iter()
            .find(|b| b.id == key)
            .is_some_and(|b| active(&paper, b)))
    }
    pub async fn pending_binding_audit(
        &self,
        caller: &Caller,
        id: &SubmissionId,
    ) -> CoreResult<Option<PendingBindingAudit>> {
        caller.require(Role::Reviewer)?;
        let paper = self.load(id).await?;
        let (file, _) = self.referee_file(id).await?;
        Ok(file
            .binding_boards
            .iter()
            .filter(|b| active(&paper, b))
            .find_map(BindingBoard::pending))
    }
    pub async fn record_binding_inspection(
        &self,
        caller: &Caller,
        job: &LeasedJob,
        lease: &dyn JobLease,
        pending: &PendingBindingAudit,
        inspection: BindingInspection,
    ) -> CoreResult<()> {
        caller.require(Role::Reviewer)?;
        if job.kind != JobKind::BindingAudit
            || job.submission != pending.board.target.subject.submission
        {
            return Err(CoreError::invalid("wrong binding audit job"));
        }
        require_text("independent binding audit reason", &inspection.reason, 5000)?;
        if inspection.evidence.len() > 20_000 || inspection.inspected_sources.len() > 40 {
            return Err(CoreError::invalid("binding audit evidence too large"));
        }
        let complete = inspection.evidence_complete
            && pending
                .contribution
                .binding
                .premises
                .iter()
                .all(|p| inspection.inspected_sources.contains(p))
            && inspection
                .inspected_sources
                .iter()
                .all(|p| pending.contribution.binding.premises.contains(p));
        let verdict = if !complete
            || inspection.evidence.trim().is_empty()
            || inspection.verdict == BindingVerdict::Supported
                && !pending.contribution.binding.discharged()
        {
            BindingVerdict::Unclear
        } else {
            inspection.verdict
        };
        loop {
            if !lease.renew(job).await? {
                return Err(CoreError::conflict("binding audit lease lost"));
            }
            let paper = self.load(&job.submission).await?;
            if paper.involves(&caller.person)
                || pending.contribution.contributor == caller.person
                || pending
                    .challenge
                    .as_ref()
                    .is_some_and(|c| c.challenger == caller.person)
            {
                return Err(CoreError::forbidden(
                    "binding inspection must be independent of the authors and contributors",
                ));
            }
            let (mut file, stored) = self.referee_file(&job.submission).await?;
            let b = board(&file, &pending.board.id)?;
            if b.audits().iter().any(|a| a.request == pending.request) {
                return Ok(());
            }
            if !active(&paper, b) {
                return Err(CoreError::conflict(
                    "binding sharing or source authorization changed",
                ));
            }
            let current = b
                .pending()
                .ok_or_else(|| CoreError::conflict("binding request settled"))?;
            if current.request != pending.request || current.board != pending.board {
                // Concurrent unrelated submissions are harmless, but never inspect an
                // out-of-order challenge or silently change the audited evidence.
                if current.request != pending.request
                    || current.contribution != pending.contribution
                    || current.challenge != pending.challenge
                    || current.board.audits() != pending.board.audits()
                {
                    return Err(CoreError::conflict("binding audit inputs changed"));
                }
            }
            let audit = BindingAudit {
                request: pending.request.clone(),
                contribution: pending.contribution.id.clone(),
                auditor: caller.person.clone(),
                verdict,
                reason: inspection.reason.clone(),
                evidence: inspection.evidence.clone(),
                inspected_sources: inspection.inspected_sources.clone(),
                evidence_complete: complete,
                at: self.ports.clock.now(),
            };
            file.binding_boards
                .iter_mut()
                .find(|b| b.id == pending.board.id)
                .unwrap()
                .events
                .push(BindingEvent::Audited { audit });
            match self.store_referee_file(&mut file, stored).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => return result,
            }
        }
    }
    pub(crate) async fn reconcile_bindings(&self) -> CoreResult<()> {
        let mut before = None;
        loop {
            let page = self.ports.referees.binding_sources(100, before).await?;
            for id in page.items {
                let Some(paper) = self.ports.submissions.get(&id).await? else {
                    continue;
                };
                let Some(file) = self.ports.referees.get(&id).await? else {
                    continue;
                };
                let cleanup = file.agent_steps.values().any(|s| {
                    s.cma_attempts.iter().any(|a| {
                        a.step.starts_with("binding:")
                            && a.progress["agent_id"].is_string()
                            && a.progress["deleted"] != true
                    }) || s.cma_attempts.last().is_some_and(|a| {
                        a.step.starts_with("binding:")
                            && s.progress["agent_id"].is_string()
                            && s.progress["deleted"] != true
                    })
                });
                if cleanup
                    || file
                        .binding_boards
                        .iter()
                        .any(|b| active(&paper, b) && b.pending().is_some())
                {
                    self.ports.queue.enqueue(&id, JobKind::BindingAudit).await?;
                }
            }
            before = page.next_before;
            if before.is_none() {
                return Ok(());
            }
        }
    }
}
