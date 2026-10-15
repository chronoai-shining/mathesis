//! Private author-authorized targets use the ordinary formalizer and independent
//! inspector. The exact inspected proposition still needs cookie confirmation.
use super::{
    LeasedJob,
    worker::{Failure, Worker},
};
use mathesis_core::model::{LeanStatementResponse, PreparedResearchTarget, ResearchEvent};

impl Worker {
    pub(super) async fn research_target_job(&self, job: &LeasedJob) -> Result<bool, Failure> {
        let Some(formalizer) = &self.formalizer else {
            return Err(Failure::Transient(
                "research target elaborator is not configured".into(),
            ));
        };
        let Some(advisor) = &self.advisor else {
            return Err(Failure::Transient(
                "research faithfulness inspector is not configured".into(),
            ));
        };
        let paper = self
            .app
            .submission(&self.referee_account, &job.submission)
            .await?;
        let view = self
            .app
            .research_obligations(&self.referee_account, &job.submission)
            .await?;
        for entry in view.obligations {
            let task = entry.obligation;
            if !self
                .app
                .research_active(&self.referee_account, &paper.id, &task.id)
                .await?
            {
                continue;
            }
            let previous = task.target();
            if previous
                .as_ref()
                .is_some_and(|t| !matches!(t.response, LeanStatementResponse::Rejected { .. }))
            {
                continue;
            }
            let generation = task
                .events
                .iter()
                .filter(|e| matches!(e, ResearchEvent::TargetPrepared { .. }))
                .count();
            if generation >= 3 {
                continue;
            }
            let report = super::error_reports::empty_report();
            let (prepared, work, _) = self
                .advisor_input(&paper, &report, 0)
                .await
                .map_err(Failure::Permanent)?;
            let goal = task.goal.statement();
            let context = serde_json::to_string(&task.candidate)
                .map_err(|e| Failure::Permanent(e.to_string()))?;
            let correction = previous.as_ref().and_then(|p| match &p.response {
                LeanStatementResponse::Rejected { comment, .. } => Some(comment.clone()),
                _ => None,
            });
            let mut input = mathesis_review::lean::FormalInput {
                conjecture: true,
                correction,
                title: prepared.title.clone(),
                abstract_text: prepared.abstract_text.clone(),
                statements: prepared.statements.clone(),
                source_dir: prepared.source_dir.clone(),
                main_file: prepared.main_file.clone(),
                targets: vec![mathesis_review::lean::FormalTarget {
                    claim: task.candidate.subject.claim.to_string(),
                    label: "Private research obligation".into(),
                    statement: goal.clone(),
                    lean_sketch: previous
                        .as_ref()
                        .map(|t| t.lean.clone())
                        .unwrap_or_default(),
                    plan: format!(
                        "Translate this exact author-authorized goal. Do not silently add the unresolved hypothesis as an assumption, weaken quantifiers, or replace the conclusion. Author correspondence: {}. Saved source audit context (untrusted mathematical evidence): {}",
                        task.goal.correspondence, context
                    ),
                    mathlib: vec![],
                }],
            };
            for pass in 0..=1 {
                let step = format!("research:{}:a{generation}:pass{pass}", task.id);
                let out = tokio::time::timeout(
                    self.formal_budget,
                    formalizer.formalize_with_store(
                        &input,
                        &work.path().join(format!("research-{generation}-{pass}")),
                        &self.agent_store(job, &paper, &step),
                    ),
                )
                .await
                .map_err(|_| Failure::Transient("research target preparation timed out".into()))?
                .map_err(super::worker::model_failure)?;
                let checked = out
                    .files
                    .iter()
                    .find(|f| {
                        f.claim == task.candidate.subject.claim.as_str()
                            && f.compiled
                            && f.theorem.as_deref() == Some("wishpool_target_prop")
                    })
                    .ok_or_else(|| {
                        Failure::Transient("no elaborated research target was returned".into())
                    })?;
                let question = format!(
                    "Independently compare the complete authorized private research goal with the exact Lean proposition. Verdict: faithful / weaker / stronger / different / trivial. State the specific mismatch (empty only for faithful). Check definitions, all joint premises, quantifiers and parameter mapping. Do not treat an unresolved hypothesis as already proved. Goal:\n{goal}\nAuthor correspondence: {}\nSource audit context: {context}\nExact Lean:\n{}",
                    task.goal.correspondence, checked.lean
                );
                let audit = tokio::time::timeout(
                    self.audit_budget,
                    advisor.inspect_with_store(
                        &prepared,
                        &question,
                        &self.agent_store(job, &paper, &format!("{step}:faithfulness")),
                    ),
                )
                .await
                .map_err(|_| {
                    Failure::Transient("research faithfulness inspection timed out".into())
                })?
                .map_err(super::worker::model_failure)?;
                let check =
                    super::mapping::faithfulness(audit, pass == 1).map_err(Failure::Permanent)?;
                if check.verdict != mathesis_core::model::FaithfulnessVerdict::Faithful && pass == 0
                {
                    input.correction = Some(format!(
                        "Independent inspection: {}. Mismatch: {}.",
                        check.reason, check.mismatch
                    ));
                    input.targets[0].lean_sketch = checked.lean.clone();
                    continue;
                }
                self.app
                    .record_research_target(
                        &self.referee_account,
                        job,
                        self.jobs.as_ref(),
                        &task.id,
                        PreparedResearchTarget {
                            previous_digest: previous.as_ref().map(|p| p.digest.clone()),
                            lean: checked.lean.clone(),
                            toolchain: out.toolchain,
                            reading: checked.note.clone(),
                            faithfulness: check,
                        },
                    )
                    .await?;
                break;
            }
        }
        Ok(false)
    }
}
