//! A bounded source index, independent of review judgements. Candidates are
//! discovered in hash-checked mathematical source, not just declaration names.
//! Model-proposed mathematical reformulations can query the same immutable index.

use super::{PublicationIdentity, PublishedDeclaration, ensure, invalid, sha256};
use crate::ReviewResult;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const INDEX_PROTOCOL: &str = "mathesis.source-index.v1";
pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_EXCERPT_BYTES: usize = 80_000;
pub const MAX_INDEX_SOURCE_BYTES: usize = 256 * 1024 * 1024;
const MAX_RESULT_SOURCE_BYTES: usize = 256_000;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceModule {
    pub path: String,
    pub source_sha256: String,
    pub source: String,
    pub declarations: Vec<PublishedDeclaration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceOmission {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceIndex {
    pub protocol: String,
    pub publication: PublicationIdentity,
    pub revision: String,
    pub candidate_module_count: usize,
    pub modules: Vec<SourceModule>,
    pub omissions: Vec<SourceOmission>,
}

impl SourceIndex {
    pub fn build(
        publication: PublicationIdentity,
        catalog: Vec<PublishedDeclaration>,
        mut source: impl FnMut(&str) -> Result<Vec<u8>, String>,
    ) -> ReviewResult<Self> {
        let mut grouped: BTreeMap<String, Vec<PublishedDeclaration>> = BTreeMap::new();
        for declaration in catalog {
            ensure(
                declaration.identity.snapshot == publication.bundle_digest,
                "index mixes publications",
            )?;
            grouped
                .entry(declaration.identity.module_path.clone())
                .or_default()
                .push(declaration);
        }
        let mut index = Self {
            protocol: INDEX_PROTOCOL.into(),
            publication,
            revision: String::new(),
            candidate_module_count: grouped.len(),
            modules: Vec::new(),
            omissions: Vec::new(),
        };
        ensure(grouped.len() <= 50_000, "too many indexed modules")?;
        let mut source_bytes = 0;
        for (path, mut declarations) in grouped {
            declarations.sort_by(|a, b| a.identity.name_key.cmp(&b.identity.name_key));
            ensure(safe_source_path(&path), "unsafe source path")?;
            let bytes = match source(&path) {
                Ok(bytes) => bytes,
                Err(_) => {
                    index.omissions.push(SourceOmission {
                        path,
                        reason: "source_unavailable".into(),
                    });
                    continue;
                }
            };
            if bytes.len() > MAX_SOURCE_BYTES {
                index.omissions.push(SourceOmission {
                    path,
                    reason: "source_exceeds_limit".into(),
                });
                continue;
            }
            source_bytes += bytes.len();
            ensure(
                source_bytes <= MAX_INDEX_SOURCE_BYTES,
                "source index exceeds total byte limit",
            )?;
            let hash = sha256(&bytes);
            // A changed source is a bad publication/input, never a silently omitted hit.
            ensure(
                declarations.iter().all(|d| d.source_sha256 == hash),
                "source checksum mismatch",
            )?;
            let source = String::from_utf8(bytes).map_err(|_| invalid("source is not UTF-8"))?;
            index.modules.push(SourceModule {
                path,
                source_sha256: hash,
                source,
                declarations,
            });
        }
        index.revision = index.content_digest()?;
        index.validate()?;
        Ok(index)
    }

    fn content_digest(&self) -> ReviewResult<String> {
        let bytes = serde_json::to_vec(&(
            &self.protocol,
            &self.publication,
            self.candidate_module_count,
            &self.modules,
            &self.omissions,
        ))
        .map_err(|_| invalid("cannot encode source index"))?;
        Ok(sha256(&bytes))
    }

    pub fn validate(&self) -> ReviewResult<()> {
        self.publication.validate()?;
        ensure(
            self.protocol == INDEX_PROTOCOL && self.revision == self.content_digest()?,
            "source index revision mismatch",
        )?;
        ensure(
            self.modules.len() + self.omissions.len() == self.candidate_module_count
                && self.candidate_module_count <= 50_000,
            "source index inventory mismatch",
        )?;
        ensure(
            self.modules.iter().map(|m| m.source.len()).sum::<usize>() <= MAX_INDEX_SOURCE_BYTES,
            "source index exceeds total byte limit",
        )?;
        let mut paths = BTreeSet::new();
        let mut declaration_ids = BTreeSet::new();
        for module in &self.modules {
            ensure(
                safe_source_path(&module.path)
                    && paths.insert(&module.path)
                    && module.source.len() <= MAX_SOURCE_BYTES
                    && sha256(module.source.as_bytes()) == module.source_sha256
                    && !module.declarations.is_empty(),
                "invalid indexed source",
            )?;
            ensure(
                module.declarations.iter().all(|d| {
                    d.identity.snapshot == self.publication.bundle_digest
                        && d.identity.module_path == module.path
                        && d.source_sha256 == module.source_sha256
                        && super::hash_id(&d.identity.statement_id)
                        && super::hash_id(&d.identity.type_sha256)
                        && super::hash_id(&d.frozen_node_id)
                        && declaration_ids.insert(&d.identity.statement_id)
                        && !d.identity.name_key.is_empty()
                        && matches!(d.freeze_status.as_str(), "frozen" | "proven-not-yet-frozen")
                }),
                "indexed declaration identity mismatch",
            )?;
        }
        for omission in &self.omissions {
            ensure(
                safe_source_path(&omission.path)
                    && paths.insert(&omission.path)
                    && matches!(
                        omission.reason.as_str(),
                        "source_unavailable" | "source_exceeds_limit"
                    ),
                "invalid index omission",
            )?;
        }
        Ok(())
    }
}

fn safe_source_path(path: &str) -> bool {
    path.ends_with(".lean")
        && !path.contains('\\')
        && path
            .split('/')
            .all(|s| !s.is_empty() && s != "." && s != "..")
}

fn terms(text: &str) -> Vec<String> {
    let mut normalized = String::new();
    for c in text.chars() {
        normalized.push_str(match c {
            '∀' => " forall ",
            '∃' => " exists ",
            '→' => " implies ",
            '↔' => " iff ",
            'ℝ' => " real ",
            'ℂ' => " complex ",
            'ℕ' => " nat ",
            'ℤ' => " int ",
            '≤' => " le ",
            '≥' => " ge ",
            '≠' => " ne ",
            _ => {
                normalized.extend(c.to_lowercase());
                continue;
            }
        });
    }
    normalized
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| {
            s.len() >= 2
                && !matches!(
                    *s,
                    "the"
                        | "and"
                        | "for"
                        | "with"
                        | "that"
                        | "this"
                        | "from"
                        | "have"
                        | "by"
                        | "of"
                        | "in"
                        | "is"
                        | "to"
                        | "let"
                        | "fun"
                        | "then"
                        | "else"
                )
        })
        .map(str::to_owned)
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceHit {
    pub module_path: String,
    pub source_sha256: String,
    pub source_excerpt: String,
    pub excerpt_truncated: bool,
    pub declarations: Vec<PublishedDeclaration>,
    pub declarations_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceQueryResult {
    pub snapshot: String,
    pub index_revision: String,
    pub searched_modules: usize,
    pub omitted_modules: usize,
    pub hits: Vec<SourceHit>,
    pub matching_modules: usize,
    pub result_truncated: bool,
    pub limits: Vec<String>,
}

pub struct SearchIndex {
    index: SourceIndex,
    postings: BTreeMap<String, Vec<(usize, usize)>>,
    lengths: Vec<usize>,
    average_length: f64,
}

impl SearchIndex {
    pub fn new(index: SourceIndex) -> ReviewResult<Self> {
        index.validate()?;
        let mut postings: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
        let mut lengths = Vec::new();
        for (i, module) in index.modules.iter().enumerate() {
            let mut counts = BTreeMap::new();
            let words = terms(&module.source);
            lengths.push(words.len());
            for word in words {
                *counts.entry(word).or_insert(0) += 1;
            }
            for (word, count) in counts {
                postings.entry(word).or_default().push((i, count));
            }
        }
        let average_length = lengths.iter().sum::<usize>() as f64 / lengths.len().max(1) as f64;
        Ok(Self {
            index,
            postings,
            lengths,
            average_length: average_length.max(1.0),
        })
    }

    pub fn identity(&self) -> (&str, &str) {
        (&self.index.publication.bundle_digest, &self.index.revision)
    }

    /// Ranking is only candidate discovery. No similarity value is exported as
    /// mathematical evidence, coverage probability or publication recommendation.
    pub fn search(
        &self,
        query: &str,
        type_digest: Option<&str>,
        limit: usize,
    ) -> ReviewResult<SourceQueryResult> {
        ensure(
            !query.trim().is_empty() && query.len() <= 30_000 && (1..=20).contains(&limit),
            "invalid source query",
        )?;
        let mut scores: BTreeMap<usize, f64> = BTreeMap::new();
        for word in terms(query).into_iter().collect::<BTreeSet<_>>() {
            if let Some(hits) = self.postings.get(&word) {
                let idf = (1.0
                    + (self.index.modules.len() as f64 - hits.len() as f64 + 0.5)
                        / (hits.len() as f64 + 0.5))
                    .ln();
                for (i, count) in hits {
                    let normalization =
                        1.2 * (0.25 + 0.75 * self.lengths[*i] as f64 / self.average_length);
                    *scores.entry(*i).or_insert(0.0) +=
                        idf * (*count as f64 * 2.2) / (*count as f64 + normalization);
                }
            }
        }
        if let Some(type_digest) = type_digest {
            ensure(super::hash_id(type_digest), "invalid structured type query")?;
            for (i, module) in self.index.modules.iter().enumerate() {
                if module
                    .declarations
                    .iter()
                    .any(|d| d.identity.type_sha256 == type_digest)
                {
                    *scores.entry(i).or_insert(0.0) += 1_000_000.0;
                }
            }
        }
        let mut scores = scores.into_iter().collect::<Vec<_>>();
        scores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let matching_modules = scores.len();
        let mut remaining = 100;
        let mut source_remaining = MAX_RESULT_SOURCE_BYTES;
        let hits: Vec<SourceHit> = scores
            .into_iter()
            .take(limit)
            .map(|(i, _)| {
                let module = &self.index.modules[i];
                let mut end = module
                    .source
                    .len()
                    .min(MAX_EXCERPT_BYTES)
                    .min(source_remaining);
                while !module.source.is_char_boundary(end) {
                    end -= 1;
                }
                source_remaining -= end;
                let count = remaining.min(module.declarations.len());
                let mut declarations = module.declarations.iter().collect::<Vec<_>>();
                declarations.sort_by_key(|d| type_digest != Some(d.identity.type_sha256.as_str()));
                remaining -= count;
                SourceHit {
                    module_path: module.path.clone(),
                    source_sha256: module.source_sha256.clone(),
                    source_excerpt: module.source[..end].into(),
                    excerpt_truncated: end < module.source.len(),
                    declarations: declarations.into_iter().take(count).cloned().collect(),
                    declarations_truncated: count < module.declarations.len(),
                }
            })
            .collect();
        Ok(SourceQueryResult {snapshot:self.index.publication.bundle_digest.clone(),index_revision:self.index.revision.clone(),
            searched_modules:self.index.modules.len(),omitted_modules:self.index.omissions.len(),
            matching_modules, result_truncated:matching_modules>hits.len() || hits.iter().any(|h| h.excerpt_truncated || h.declarations_truncated),hits,
            limits:vec!["Full-source term retrieval and optional exact type identity discover candidates; they do not establish entailment or exhaust mathematical reformulations.".into(),
                "Canonical declaration identities come from the checked publication; individual canonical materials require their separate bounded read before claiming material validation.".into()]})
    }
}
