//! The worker files exact results under the original source and lease fences.
use super::App;
use crate::{
    CoreError, CoreResult,
    model::*,
    ports::{JobLease, LeasedJob},
};

impl App {
    pub async fn record_computation_leased(
        &self,
        caller: &Caller,
        job: &LeasedJob,
        leases: &dyn JobLease,
        receipt: ComputationReceipt,
    ) -> CoreResult<RefereeFile> {
        caller.require(Role::Reviewer)?;
        receipt.validate()?;
        if job.kind != crate::ports::JobKind::Referee {
            return Err(CoreError::conflict(
                "finite replay requires a referee lease",
            ));
        }
        let submission = self.submission(caller, &job.submission).await?;
        if submission.involves(&caller.person) {
            return Err(CoreError::forbidden(
                "authors cannot file their own computation receipts",
            ));
        }
        let (mut file, stored) = self.referee_file(&job.submission).await?;
        let round = file
            .rounds
            .last_mut()
            .filter(|r| r.number == receipt.round)
            .ok_or_else(|| CoreError::conflict("computation must name the current round"))?;
        if round.computation_receipts.contains(&receipt) {
            return Ok(file);
        }
        let version = submission
            .current_version()
            .ok_or_else(|| CoreError::conflict("source missing"))?;
        if round.computation_contract != 1
            || !matches!(round.referee.state, StepState::Pending)
            || round.referee.client_ref.is_some()
            || round.version != receipt.version
            || version.number != receipt.version
            || round.claims_revision != receipt.claims_revision
            || submission.claims_revision != receipt.claims_revision
            || round.source_sha256 != receipt.source_sha256
            || version.archive.sha256 != receipt.source_sha256
            || !version.evidence.contains(&receipt.attachment)
            || receipt.attachment.descriptor.replay_protocol.as_deref()
                != Some(COMPUTATION_PROTOCOL)
        {
            return Err(CoreError::conflict(
                "computation inputs changed or referee already admitted",
            ));
        }
        if round.computation_receipts.len() >= 100
            || round
                .computation_receipts
                .iter()
                .any(|r| r.attachment.descriptor.path == receipt.attachment.descriptor.path)
        {
            return Err(CoreError::conflict(
                "finite computation receipt is immutable or limit reached",
            ));
        }
        round.computation_receipts.push(receipt);
        if !leases.renew(job).await? {
            return Err(CoreError::conflict("computation lost its lease"));
        }
        self.store_referee_file(&mut file, stored).await?;
        Ok(file)
    }
}
