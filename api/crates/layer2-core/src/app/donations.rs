//! Donated quota: the donor's view and the hosted worker's accounting.

use super::App;
use crate::{
    CoreError, CoreResult,
    ids::PersonId,
    model::{Caller, DonationGrant, GrantStatus, period_of},
};

impl App {
    pub async fn donation(&self, caller: &Caller) -> CoreResult<Option<DonationGrant>> {
        self.ports.grants.get(&caller.person).await
    }

    /// The donor changes their cap or pauses, resumes or revokes the grant.
    pub async fn update_donation(
        &self,
        caller: &Caller,
        monthly_cap: Option<u64>,
        status: Option<GrantStatus>,
    ) -> CoreResult<DonationGrant> {
        let mut grant = self
            .ports
            .grants
            .get(&caller.person)
            .await?
            .ok_or_else(|| CoreError::not_found("donation", caller.person.as_str()))?;
        if grant.status == GrantStatus::Revoked {
            return Err(CoreError::conflict(
                "a revoked grant is renewed by authorizing again",
            ));
        }
        if let Some(cap) = monthly_cap {
            DonationGrant::validate_cap(cap)?;
            grant.monthly_cap = cap;
        }
        if let Some(status) = status {
            grant.status = status;
        }
        let expected = grant.revision;
        grant.updated_at = self.ports.clock.now();
        grant.revision = expected + 1;
        self.ports.grants.replace(&grant, expected).await?;
        Ok(grant)
    }

    /// Composition-only: record a grant after the donor authorized the
    /// venue in NyxID and the delegated credential was stored.
    pub async fn create_or_renew_donation(
        &self,
        donor: &PersonId,
        monthly_cap: u64,
        model: &str,
    ) -> CoreResult<DonationGrant> {
        DonationGrant::validate_cap(monthly_cap)?;
        let now = self.ports.clock.now();
        match self.ports.grants.get(donor).await? {
            Some(mut grant) => {
                let expected = grant.revision;
                grant.monthly_cap = monthly_cap;
                grant.model = model.to_owned();
                grant.status = GrantStatus::Active;
                grant.updated_at = now;
                grant.revision = expected + 1;
                self.ports.grants.replace(&grant, expected).await?;
                Ok(grant)
            }
            None => {
                let grant = DonationGrant {
                    runs: Default::default(),
                    donor: donor.clone(),
                    monthly_cap,
                    model: model.to_owned(),
                    period: period_of(now),
                    used: 0,
                    status: GrantStatus::Active,
                    created_at: now,
                    updated_at: now,
                    revision: 0,
                };
                self.ports.grants.insert(&grant).await?;
                Ok(grant)
            }
        }
    }

    /// Composition-only: grants with budget left this month.
    pub async fn spendable_donations(&self) -> CoreResult<Vec<DonationGrant>> {
        let now = self.ports.clock.now();
        Ok(self
            .ports
            .grants
            .active()
            .await?
            .into_iter()
            .filter(|g| g.remaining(now) > 0)
            .collect())
    }

    /// Composition-only: charge metered usage to a grant.
    pub async fn charge_donation(
        &self,
        donor: &PersonId,
        tokens: u64,
    ) -> CoreResult<DonationGrant> {
        for _ in 0..3 {
            let mut grant = self
                .ports
                .grants
                .get(donor)
                .await?
                .ok_or_else(|| CoreError::not_found("donation", donor.as_str()))?;
            let expected = grant.revision;
            grant.charge(tokens, self.ports.clock.now());
            grant.revision = expected + 1;
            match self.ports.grants.replace(&grant, expected).await {
                Ok(()) => return Ok(grant),
                Err(CoreError::StaleRevision { .. }) => continue,
                Err(error) => return Err(error),
            }
        }
        Err(CoreError::Unavailable(
            "could not record usage after three attempts".into(),
        ))
    }

    /// Composition-only: revoke when the delegated credential stops working.
    pub async fn revoke_donation(&self, donor: &PersonId) -> CoreResult<()> {
        if let Some(mut grant) = self.ports.grants.get(donor).await? {
            let expected = grant.revision;
            grant.status = GrantStatus::Revoked;
            grant.updated_at = self.ports.clock.now();
            grant.revision = expected + 1;
            self.ports.grants.replace(&grant, expected).await?;
        }
        Ok(())
    }
}

impl App {
    /// Bind a venue CMA run to a supporter once. Attribution is not a usage receipt.
    pub async fn attribute_agent_run(&self, run: &str) -> CoreResult<Option<PersonId>> {
        loop {
            let grants = self.ports.grants.all_grants().await?;
            if let Some(g) = grants.iter().find(|g| g.runs.contains_key(run)) {
                return Ok(Some(g.donor.clone()));
            }
            let Some(mut grant) = grants
                .into_iter()
                .filter(|g| g.remaining(self.ports.clock.now()) > 0)
                .min_by_key(|g| (g.runs.len(), g.donor.clone()))
            else {
                return Ok(None);
            };
            let expected = grant.revision;
            grant.runs.insert(
                run.into(),
                crate::model::FundedRun {
                    attributed_at: self.ports.clock.now(),
                    usage: None,
                },
            );
            grant.revision += 1;
            match self.ports.grants.replace(&grant, expected).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => result?,
            }
            return Ok(Some(grant.donor));
        }
    }
    /// Composition-only NyxID accounting boundary. Never called with model output or client claims.
    /// A run's authoritative usage is immutable and charged in the same CAS as its receipt.
    pub async fn record_funded_usage(
        &self,
        donor: &PersonId,
        run: &str,
        usage: crate::model::TokenUsage,
    ) -> CoreResult<()> {
        if !usage.metered {
            return Err(CoreError::invalid(
                "only NyxID-reported usage may be metered",
            ));
        }
        loop {
            let mut grant = self
                .ports
                .grants
                .get(donor)
                .await?
                .ok_or_else(|| CoreError::not_found("donation", donor.as_str()))?;
            let funded = grant
                .runs
                .get_mut(run)
                .ok_or_else(|| CoreError::not_found("funded run", run))?;
            if let Some(old) = funded.usage {
                return if old == usage {
                    Ok(())
                } else {
                    Err(CoreError::conflict("usage receipt is immutable"))
                };
            }
            funded.usage = Some(usage);
            let expected = grant.revision;
            grant.charge(usage.total(), self.ports.clock.now());
            grant.revision += 1;
            match self.ports.grants.replace(&grant, expected).await {
                Err(CoreError::StaleRevision { .. }) => continue,
                result => return result,
            }
        }
    }
    pub async fn revoked_donations(&self) -> CoreResult<Vec<PersonId>> {
        Ok(self
            .ports
            .grants
            .all_grants()
            .await?
            .into_iter()
            .filter(|g| g.status == GrantStatus::Revoked)
            .map(|g| g.donor)
            .collect())
    }
    pub async fn supporters(&self) -> CoreResult<Vec<crate::model::Supporter>> {
        let mut supporters = vec![];
        for g in self.ports.grants.all_grants().await? {
            if g.status == GrantStatus::Revoked && g.runs.is_empty() {
                continue;
            }
            if let Some(person) = self.ports.people.get(&g.donor).await? {
                supporters.push(crate::model::Supporter {
                    name: person.display_name,
                    tokens: g
                        .runs
                        .values()
                        .filter_map(|r| r.usage)
                        .filter(|u| u.metered)
                        .map(|u| u.total())
                        .fold(0u64, u64::saturating_add),
                });
            }
        }
        supporters.sort_by_key(|s| s.name.to_lowercase());
        Ok(supporters)
    }
}
