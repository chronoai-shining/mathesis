//! Composition of actual read-only GitHub facts and an already admitted index.
use super::{
    LeasedJob,
    worker::{Failure, Worker},
};
use mathesis_core::model::*;
use mathesis_review::knowledge::{self, DeclarationIdentity};
use serde_json::json;
impl Worker {
    pub(super) async fn feedback_readback_job(&self, job: &LeasedJob) -> Result<bool, Failure> {
        while let Some(pending) = self
            .app
            .pending_feedback_readback(&self.reviewer, &job.submission)
            .await?
        {
            self.ensure_lease(job).await?;
            let requested = pending
                .request
                .declarations
                .iter()
                .map(|d| DeclarationIdentity {
                    snapshot: d.snapshot.clone(),
                    module_path: d.module_path.clone(),
                    name_key: d.declaration_name_key.clone(),
                    statement_id: d.statement_id.clone(),
                    type_sha256: d.type_digest.clone(),
                })
                .collect::<Vec<_>>();
            let mut observation = FeedbackObservation {
                request: pending.request_id.clone(),
                pull_request: pending.request.pull_request,
                upstream_repository: "the-omega-institute/trureturing".into(),
                upstream_base: String::new(),
                reference_state: "unavailable".into(),
                merge_commit: None,
                merged_at: None,
                consumed_source_commit: String::new(),
                consumed_source_tree: String::new(),
                snapshot: String::new(),
                index_revision: pending.request.index_revision.clone(),
                merge_in_consumed_source: false,
                declarations: pending.request.declarations.iter().cloned().map(|identity| FeedbackDeclarationObservation {
                    identity, present: false, source_sha256: None,
                    freeze_status: None, frozen_node_id: None, package_marker_present: false,
                }).collect(),
                evidence: BlobRef {id:String::new(), bytes:0, sha256:String::new()},
                observed_at: chrono::Utc::now(),
                limits: vec![
                    "Declaration presence and explicit module provenance do not prove mathematical correspondence, upstream registration, or a recognized coverage certificate.".into(),
                    "The bounded admitted index can omit modules; an absent exact identity does not establish that a declaration does not exist.".into(),
                ],
            };
            let mut raw =
                json!({"request":pending.request,"proposal_digest":pending.proposal.digest});
            if let Some(source) = &self.knowledge {
                let marker = format!("Mathesis-Proposal: {}", pending.proposal.digest);
                match source
                    .observe_declarations(&pending.request.index_revision, &requested, &marker)
                    .await
                {
                    Ok((publication, declarations)) => {
                        observation.consumed_source_commit = publication.source_commit.clone();
                        observation.consumed_source_tree = publication.source_tree.clone();
                        observation.snapshot = publication.bundle_digest.clone();
                        if declarations.len() != observation.declarations.len() {
                            return Err(Failure::Permanent(
                                "declaration observation count mismatch".into(),
                            ));
                        }
                        raw["publication"] = json!(publication);
                        raw["declarations"] = json!(declarations);
                        for ((found, marker), entry) in
                            declarations.into_iter().zip(&mut observation.declarations)
                        {
                            if let Some(d) = found {
                                let expected = DeclarationIdentity {
                                    snapshot: entry.identity.snapshot.clone(),
                                    module_path: entry.identity.module_path.clone(),
                                    name_key: entry.identity.declaration_name_key.clone(),
                                    statement_id: entry.identity.statement_id.clone(),
                                    type_sha256: entry.identity.type_digest.clone(),
                                };
                                if d.identity != expected
                                    || d.identity.snapshot != publication.bundle_digest
                                {
                                    return Err(Failure::Permanent(
                                        "admitted declaration identity mismatch".into(),
                                    ));
                                }
                                entry.present = true;
                                entry.source_sha256 = Some(d.source_sha256);
                                entry.freeze_status = Some(d.freeze_status);
                                entry.frozen_node_id = Some(d.frozen_node_id);
                                entry.package_marker_present = marker;
                            }
                        }
                        // The publication adapter uses a fixed read-only host/repository,
                        // exact admitted commit/tree and bounded response bodies.
                        let git = knowledge::readback::observe(
                            pending.request.pull_request,
                            &publication,
                        )
                        .await;
                        raw["github"] = json!(git);
                        observation.upstream_repository = git.repository;
                        observation.upstream_base = git.base;
                        observation.reference_state = git.state;
                        observation.merge_commit = git.merge_commit;
                        observation.merged_at = git
                            .merged_at
                            .and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok())
                            .map(|t| t.with_timezone(&chrono::Utc));
                        observation.merge_in_consumed_source = git.included;
                        observation.limits.extend(git.limits);
                    }
                    Err(_) => {
                        observation.limits.push("Requested immutable admitted index could not be read and checked; no upstream claim was observed.".into());
                        raw["index_error"] = json!("admitted index unavailable");
                    }
                }
            } else {
                observation
                    .limits
                    .push("No admitted knowledge source is configured for observation.".into());
                raw["index_error"] = json!("knowledge source unavailable");
            }
            observation.observed_at = chrono::Utc::now();
            let result = InspectedFeedback {
                observation,
                evidence: serde_json::to_vec(&raw)
                    .map_err(|_| Failure::Permanent("cannot encode observation evidence".into()))?,
            };
            self.ensure_lease(job).await?;
            self.app
                .record_feedback_observation(
                    &self.reviewer,
                    job,
                    self.jobs.as_ref(),
                    &pending,
                    result,
                )
                .await?;
        }
        Ok(false)
    }
}
