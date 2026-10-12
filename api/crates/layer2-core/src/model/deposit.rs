//! Author-supplied formalization sources, distinct from verifier receipts.
use super::{BlobRef, Source, require_text};
use crate::{CoreError, CoreResult, ids::PersonId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewLeanDeposit {
    pub title: String,
    pub authors: Vec<String>,
    pub statement: String,
    pub source_reference: Source,
    pub toolchain: String,
    pub license: String,
    pub solution: String,
}
impl NewLeanDeposit {
    pub fn validate(&self) -> CoreResult<()> {
        require_text("title", &self.title, 1000)?;
        require_text("statement", &self.statement, 20000)?;
        require_text("toolchain", &self.toolchain, 200)?;
        require_text("license", &self.license, 200)?;
        if self.authors.is_empty() || self.authors.len() > 50 {
            return Err(CoreError::invalid("supply 1 to 50 original authors"));
        }
        for author in &self.authors {
            require_text("author", author, 200)?;
        }
        require_text("source locator", &self.source_reference.locator, 2000)?;
        self.source_reference.validate()?;
        if self.solution.trim().is_empty() || self.solution.len() > 1_048_576 {
            return Err(CoreError::invalid("Lean source must be 1 byte to 1 MB"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeanDeposit {
    pub id: String,
    pub owner: PersonId,
    pub title: String,
    pub authors: Vec<String>,
    pub statement: String,
    pub source_reference: Source,
    pub toolchain: String,
    pub license: String,
    pub source: BlobRef,
    pub submitted_at: DateTime<Utc>,
    pub attempts: Vec<String>,
    pub revision: u64,
}

/// Reported provenance travels with a verified proof; it is not a verified authorship claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormalizationAttribution {
    pub deposit: String,
    pub authors: Vec<String>,
    pub source_reference: Source,
    pub license: String,
}
