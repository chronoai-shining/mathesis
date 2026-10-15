//! Private, version-bound finite calculations performed by the worker.
use super::EvidenceAttachment;
use crate::{CoreError, CoreResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const COMPUTATION_PROTOCOL: &str = "mathesis.rational-projection-amgm.v1";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationState {
    Counterexample,
    NotCounterexample,
    InvalidInput,
    ArithmeticLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationReceipt {
    pub protocol: String,
    pub round: u32,
    pub version: u32,
    pub claims_revision: u64,
    pub source_sha256: String,
    pub attachment: EvidenceAttachment,
    pub state: ComputationState,
    pub dimension: Option<usize>,
    pub numerator: Option<String>,
    pub denominator: Option<String>,
    pub explanation: String,
    pub checked_at: DateTime<Utc>,
}
impl ComputationReceipt {
    pub fn validate(&self) -> CoreResult<()> {
        if self.protocol != COMPUTATION_PROTOCOL
            || self.explanation.is_empty()
            || self.explanation.len() > 4000
            || self.dimension.is_some_and(|d| !(1..=6).contains(&d))
        {
            return Err(CoreError::invalid("invalid finite computation receipt"));
        }
        match self.state {
            ComputationState::Counterexample | ComputationState::NotCounterexample => {
                let numerator = self.numerator.as_ref().and_then(|s| s.parse::<i128>().ok());
                let denominator = self
                    .denominator
                    .as_ref()
                    .and_then(|s| s.parse::<i128>().ok());
                if self.dimension.is_none()
                    || numerator.is_none()
                    || denominator.is_none_or(|d| d <= 0)
                    || numerator.is_some_and(|n| {
                        (n < 0) != (self.state == ComputationState::Counterexample)
                    })
                {
                    return Err(CoreError::invalid(
                        "finite computation value contradicts its state",
                    ));
                }
            }
            _ if self.numerator.is_some() || self.denominator.is_some() => {
                return Err(CoreError::invalid(
                    "failed computation cannot contain a result",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}
