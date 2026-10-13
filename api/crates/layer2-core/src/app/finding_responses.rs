//! Author responses request an ordinary, version-bound re-review; they never resolve findings themselves.
use super::App;
use crate::{
    CoreError, CoreResult,
    ids::SubmissionId,
    model::*,
    ports::{JobKind, SubmissionFilter},
};
use sha2::{Digest, Sha256};
impl App {
    pub async fn respond_to_finding(
        &self,
        caller: &Caller,
        id: &SubmissionId,
        new: NewFindingResponse,
    ) -> CoreResult<RefereeView> {
        let submission = self.submission(caller, id).await?;
        if !submission.involves(&caller.person) {
            return Err(CoreError::forbidden("only authors may respond to findings"));
        }
        if matches!(
            submission.status,
            SubmissionStatus::Draft | SubmissionStatus::Withdrawn
        ) || submission.current_version().map(|v| v.number) != Some(new.version)
            || submission.claims_revision != new.claims_revision
        {
            return Err(CoreError::conflict(
                "confirm the current source and statements before requesting review",
            ));
        }
        require_text("response", &new.body, 10_000)?;
        let key = format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&(id, &caller.person, &new))
                    .map_err(|_| CoreError::invalid("response"))?
            )
        );
        let trigger = format!("finding-response:{key}");
        loop {
            let (mut file, stored) = self.referee_file(id).await?;
            if file.finding_responses.iter().any(|r| r.id == key) {
                self.request_recheck(id, &trigger).await?;
                return self.referee(caller, id).await;
            }
            let original = file
                .rounds
                .iter()
                .find(|r| r.number == new.round)
                .and_then(|r| r.audit.done())
                .and_then(|a| a.concerns.get(new.concern));
            if original.is_none() {
                return Err(CoreError::not_found(
                    "audit finding",
                    format!("{}:{}", new.round, new.concern),
                ));
            }
            let now = self.ports.clock.now();
            if file.finding_responses.len() >= 200
                || file
                    .finding_responses
                    .iter()
                    .filter(|r| r.at > now - chrono::Duration::hours(24))
                    .count()
                    >= 5
            {
                return Err(CoreError::conflict(
                    "at most five recheck responses per paper in 24 hours and 200 in total",
                ));
            }
            file.finding_responses.push(FindingResponse {
                id: key.clone(),
                round: new.round,
                concern: new.concern,
                version: new.version,
                claims_revision: new.claims_revision,
                body: new.body.clone(),
                author: caller.person.clone(),
                at: now,
            });
            // Persist the response and its queue intent together. Reconciliation repairs enqueue failure.
            file.recheck_requests.push(trigger.clone());
            match self.store_referee_file(&mut file, stored).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => {
                    result?;
                    break;
                }
            }
        }
        self.ports.queue.enqueue(id, JobKind::Referee).await?;
        self.referee(caller, id).await
    }
    pub(crate) async fn reconcile_private_rechecks(&self) -> CoreResult<()> {
        for status in ["in_review", "not_accepted", "review_complete"] {
            let mut cursor = None;
            loop {
                let page = self
                    .ports
                    .submissions
                    .list(
                        &SubmissionFilter {
                            status: Some(status.into()),
                            ..Default::default()
                        },
                        100,
                        cursor,
                    )
                    .await?;
                for s in page.items {
                    if let Some(file) = self.ports.referees.get(&s.id).await?
                        && !file.finding_responses.is_empty()
                        && file.current().is_none_or(|r| {
                            r.recheck_requests != file.recheck_requests || !r.is_settled()
                        })
                    {
                        self.ports.queue.enqueue(&s.id, JobKind::Referee).await?;
                    }
                }
                cursor = page.next_before;
                if cursor.is_none() {
                    break;
                }
            }
        }
        Ok(())
    }
}
