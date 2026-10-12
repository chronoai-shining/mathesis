use super::{App, clamp};
use crate::{
    CoreError, CoreResult,
    ids::{ClaimId, RecordId},
    model::*,
    ports::Listing,
};
use sha2::{Digest, Sha256};
impl App {
    pub async fn submit_lean_deposit(
        &self,
        caller: &Caller,
        new: NewLeanDeposit,
    ) -> CoreResult<LeanDeposit> {
        new.validate()?;
        // Same owner and complete metadata/source body replay to the same durable deposit.
        let hash = Sha256::digest(
            serde_json::to_vec(&new).map_err(|_| CoreError::invalid("deposit metadata"))?,
        );
        let id = format!(
            "lean-{:x}",
            Sha256::digest(format!("{}:{hash:x}", caller.person))
        );
        if let Some(existing) = self.ports.deposits.get(&id).await? {
            return Ok(existing);
        }
        let source = self
            .ports
            .blobs
            .put(new.solution.as_bytes(), "text/plain; charset=utf-8")
            .await?;
        let deposit = LeanDeposit {
            id: id.clone(),
            owner: caller.person.clone(),
            title: new.title,
            authors: new.authors,
            statement: new.statement,
            source_reference: new.source_reference,
            toolchain: new.toolchain,
            license: new.license,
            source,
            submitted_at: self.ports.clock.now(),
            attempts: vec![],
            revision: 0,
        };
        match self.ports.deposits.insert(&deposit).await {
            Ok(()) => Ok(deposit),
            Err(CoreError::Conflict(_)) => self
                .ports
                .deposits
                .get(&id)
                .await?
                .ok_or_else(|| CoreError::conflict("deposit admission conflict")),
            Err(e) => Err(e),
        }
    }
    pub async fn lean_deposit(&self, caller: &Caller, id: &str) -> CoreResult<LeanDeposit> {
        self.ports
            .deposits
            .get(id)
            .await?
            .filter(|d| d.owner == caller.person || Self::is_staff(caller))
            .ok_or_else(|| CoreError::not_found("deposit", id))
    }
    pub async fn lean_deposits(
        &self,
        caller: &Caller,
        limit: Option<u32>,
        before: Option<String>,
    ) -> CoreResult<Listing<LeanDeposit>> {
        self.ports
            .deposits
            .list(&caller.person, clamp(limit), before)
            .await
    }
    pub async fn lean_deposit_source(&self, caller: &Caller, id: &str) -> CoreResult<String> {
        let d = self.lean_deposit(caller, id).await?;
        let bytes = self
            .ports
            .blobs
            .get(&d.source.id)
            .await?
            .ok_or_else(|| CoreError::Unavailable("Lean source is missing".into()))?;
        String::from_utf8(bytes)
            .map_err(|_| CoreError::Unavailable("stored Lean source is not UTF-8".into()))
    }
    pub async fn verify_lean_deposit(
        &self,
        caller: &Caller,
        id: &str,
        record: &RecordId,
        claim: &ClaimId,
    ) -> CoreResult<AttemptView> {
        let deposit = self.lean_deposit(caller, id).await?;
        if deposit.owner != caller.person {
            return Err(CoreError::forbidden(
                "only the depositor can submit a verification attempt",
            ));
        }
        let target = self.target(record, claim).await?;
        if target.toolchain != deposit.toolchain {
            return Err(CoreError::invalid(
                "deposit toolchain must match the confirmed target toolchain",
            ));
        }
        let source = self.lean_deposit_source(caller, id).await?;
        let attribution = FormalizationAttribution {
            deposit: id.into(),
            authors: deposit.authors.clone(),
            source_reference: deposit.source_reference.clone(),
            license: deposit.license.clone(),
        };
        let attempt = self
            .submit_attributed_attempt(
                caller,
                record,
                claim,
                super::solving::AttemptInput {
                    expected_target: Some(target.digest),
                    solution: source,
                    as_agent: None,
                    note: format!("Formalization deposit {id}"),
                    attribution: Some(attribution),
                },
            )
            .await?;
        loop {
            let mut d = self.lean_deposit(caller, id).await?;
            if d.attempts.contains(&attempt.id) {
                return Ok(attempt);
            }
            let expected = d.revision;
            d.attempts.push(attempt.id.clone());
            d.revision += 1;
            match self.ports.deposits.replace(&d, expected).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => {
                    result?;
                    return Ok(attempt);
                }
            }
        }
    }
}
