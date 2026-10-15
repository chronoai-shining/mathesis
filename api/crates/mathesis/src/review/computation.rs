//! Replay declared finite certificates before admitting any model prompt.
use super::worker::{Failure, Worker};
use mathesis_core::{model::*, ports::LeasedJob};
use mathesis_review::computation;

impl Worker {
    pub(super) async fn replay_computations(
        &self,
        job: &LeasedJob,
        paper: &Submission,
        number: u32,
    ) -> Result<(), Failure> {
        let view = self.app.referee(&self.referee_account, &paper.id).await?;
        let round = view
            .rounds
            .iter()
            .find(|r| r.number == number)
            .ok_or_else(|| Failure::Permanent("computation round missing".into()))?;
        if round.computation_contract != 1 {
            return Ok(());
        }
        let version = paper
            .versions
            .iter()
            .find(|v| v.number == round.version)
            .ok_or_else(|| Failure::Permanent("computation source missing".into()))?;
        let pending = version
            .evidence
            .iter()
            .filter(|e| {
                e.descriptor.replay_protocol.is_some()
                    && !round
                        .computation_receipts
                        .iter()
                        .any(|r| r.attachment == **e)
            })
            .collect::<Vec<_>>();
        if pending.is_empty() {
            return Ok(());
        }
        let source = self
            .app
            .paper_file(
                Some(&self.referee_account),
                &paper.id,
                Some(round.version),
                false,
            )
            .await?;
        let files = mathesis_latex::archive::unpack(
            &source.bytes,
            &source.filename,
            mathesis_latex::archive::Limits::default(),
        )
        .map_err(|e| Failure::Permanent(e.to_string()))?;
        if evidence_inventory(&files)? != version.evidence {
            return Err(Failure::Permanent(
                "computation evidence inventory changed".into(),
            ));
        }
        for attachment in pending {
            self.ensure_lease(job).await?;
            let bytes = files
                .get(&attachment.descriptor.path)
                .ok_or_else(|| Failure::Permanent("declared computation file missing".into()))?;
            let result = computation::replay(bytes);
            let state = match result.state {
                computation::ReplayState::Counterexample => ComputationState::Counterexample,
                computation::ReplayState::NotCounterexample => ComputationState::NotCounterexample,
                computation::ReplayState::InvalidInput => ComputationState::InvalidInput,
                computation::ReplayState::ArithmeticLimit => ComputationState::ArithmeticLimit,
            };
            self.app
                .record_computation_leased(
                    &self.referee_account,
                    job,
                    self.jobs.as_ref(),
                    ComputationReceipt {
                        protocol: result.protocol,
                        round: number,
                        version: round.version,
                        claims_revision: round.claims_revision,
                        source_sha256: round.source_sha256.clone(),
                        attachment: attachment.clone(),
                        state,
                        dimension: result.dimension,
                        numerator: result.numerator,
                        denominator: result.denominator,
                        explanation: result.explanation,
                        checked_at: chrono::Utc::now(),
                    },
                )
                .await?;
        }
        Ok(())
    }

    pub(super) async fn computation_context(
        &self,
        paper: &Submission,
        number: u32,
    ) -> Result<Option<serde_json::Value>, Failure> {
        let view = self.app.referee(&self.referee_account, &paper.id).await?;
        let round = view
            .rounds
            .iter()
            .find(|r| r.number == number)
            .ok_or_else(|| Failure::Permanent("computation round missing".into()))?;
        if round.computation_contract == 0 || round.computation_receipts.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            serde_json::json!({"receipts":round.computation_receipts,
            "scope":"Independent exact finite arithmetic on the saved attachment bytes. This is not Lean verification, formal coverage, or proof of manuscript faithfulness. Declared claim references are author-provided. Audit whether the specific calculated assertion applies to each confirmed claim; a nonnegative witness proves no universal inequality."}),
        ))
    }
}
