//! Bounded public archive search. Private metadata never determines a match.
use super::{Record, SubmissionKind};
use crate::{CoreError, CoreResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaperSearch {
    pub q: Option<String>,
    pub author: Option<String>,
    pub msc: Option<String>,
    pub kind: Option<SubmissionKind>,
}
impl PaperSearch {
    pub fn validate(&mut self) -> CoreResult<()> {
        for (name, value, max) in [
            ("q", &mut self.q, 300),
            ("author", &mut self.author, 160),
            ("msc", &mut self.msc, 5),
        ] {
            if let Some(text) = value {
                *text = text.trim().to_owned();
                if text.chars().count() > max || text.chars().any(char::is_control) {
                    return Err(CoreError::invalid(format!(
                        "{name} is too long or contains control characters"
                    )));
                }
                if text.is_empty() {
                    *value = None;
                }
            }
        }
        if self
            .msc
            .as_ref()
            .is_some_and(|s| !s.chars().all(|c| c.is_ascii_alphanumeric()))
        {
            return Err(CoreError::invalid(
                "msc must be a classification prefix, e.g. 11 or 11A",
            ));
        }
        if self
            .q
            .as_ref()
            .is_some_and(|q| !q.chars().any(char::is_alphanumeric))
        {
            return Err(CoreError::invalid("q must contain a word or identifier"));
        }
        Ok(())
    }
    pub fn is_empty(&self) -> bool {
        self.q.is_none() && self.author.is_none() && self.msc.is_none() && self.kind.is_none()
    }
    /// The memory adapter's plain-word search; production uses MongoDB's text index.
    pub fn matches_record(&self, r: &Record) -> bool {
        if self.kind.is_some_and(|k| k != r.kind) {
            return false;
        }
        if self.author.as_ref().is_some_and(|a| {
            !r.authors
                .iter()
                .any(|v| v.name.to_lowercase().contains(&a.to_lowercase()))
        }) {
            return false;
        }
        if self.msc.as_ref().is_some_and(|m| {
            !r.publication
                .as_ref()
                .is_some_and(|s| s.msc.iter().any(|v| v.starts_with(m)))
        }) {
            return false;
        }
        let Some(q) = &self.q else {
            return true;
        };
        if q.starts_with("WP-") {
            return r.id.as_str() == q;
        }
        let mut text = format!(
            "{} {} {}",
            r.id,
            r.title,
            r.authors
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        );
        if let Some(s) = &r.publication {
            text.push_str(&format!(
                " {} {}",
                s.abstract_text,
                s.doi.as_deref().unwrap_or("")
            ));
        }
        let words: Vec<_> = text
            .split(|c: char| !c.is_alphanumeric())
            .map(str::to_lowercase)
            .collect();
        q.split(|c: char| !c.is_alphanumeric())
            .filter(|v| !v.is_empty())
            .any(|w| words.contains(&w.to_lowercase()))
    }
}
