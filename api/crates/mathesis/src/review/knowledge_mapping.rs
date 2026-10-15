//! Model proposals resolve only against the current round's actual receipt and
//! declaration identities. Identity, issuer and evidence grade are system-owned.

use mathesis_core::{ids::PersonId, model::*};
use mathesis_review::{advisor::AdvisorInput, referee_prompts::AuditOut};
use serde::Deserialize;

#[cfg(test)]
mod tests;

#[derive(Deserialize)]
struct Comparison {
    receipt_ids: Vec<String>,
    relation: CoverageRelation,
    binding: Option<Binding>,
    summary: String,
    #[serde(default)]
    limits: Vec<String>,
}

#[derive(Deserialize)]
struct Binding {
    premise_statement_ids: Vec<String>,
    relation: BindingRelation,
    parameters: Vec<ParameterBinding>,
    hypotheses: Vec<HypothesisDischarge>,
    argument: String,
    unresolved: Vec<String>,
}

fn comparison(
    raw: serde_json::Value,
    claim: &Claim,
    receipts: &[KnowledgeQueryReceipt],
    auditor: &PersonId,
    visible: &std::collections::BTreeMap<String, std::collections::BTreeSet<String>>,
) -> Result<ClaimCoverage, ()> {
    let raw: Comparison = serde_json::from_value(raw).map_err(|_| ())?;
    if raw.receipt_ids.is_empty() || raw.receipt_ids.len() > 40 {
        return Err(());
    }
    let selected = raw
        .receipt_ids
        .iter()
        .map(|id| {
            receipts
                .iter()
                .find(|r| r.id == *id && r.subject.claim == claim.id)
                .ok_or(())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let subject = selected[0].subject.clone();
    let binding = raw
        .binding
        .map(|b| {
            let premises = b
                .premise_statement_ids
                .iter()
                .map(|id| {
                    if !selected
                        .iter()
                        .any(|r| visible.get(&r.id).is_some_and(|ids| ids.contains(id)))
                    {
                        return Err(());
                    }
                    selected
                        .iter()
                        .flat_map(|r| &r.returned)
                        .find(|r| r.statement_id == *id)
                        .cloned()
                        .ok_or(())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<KnowledgeBinding, ()>(KnowledgeBinding {
                premises,
                relation: b.relation,
                parameters: b.parameters,
                hypotheses: b.hypotheses,
                argument: b.argument,
                unresolved: b.unresolved,
                source_lineage: vec![],
            })
        })
        .transpose()?;
    let qualification = if selected
        .iter()
        .all(|r| r.response.is_some() && visible.contains_key(&r.id))
    {
        CoverageQualification::SourceAudited {
            auditor: auditor.clone(),
            audit_id: format!(
                "referee-audit:{}:r{}:v{}:c{}",
                subject.submission, subject.round, subject.version, subject.claims_revision
            ),
        }
    } else {
        CoverageQualification::Candidate
    };
    let coverage = ClaimCoverage {
        subject,
        receipt_ids: raw.receipt_ids,
        relation: raw.relation,
        qualification,
        binding,
        summary: raw.summary,
        limits: raw.limits,
    };
    coverage.validate(receipts).map_err(|_| ())?;
    Ok(coverage)
}

pub(super) fn audit(
    raw: AuditOut,
    input: &AdvisorInput,
    claims: &[Claim],
    receipts: &[KnowledgeQueryReceipt],
    auditor: &PersonId,
) -> mathesis_review::ReviewResult<RefereeAudit> {
    let proposals = raw
        .claims
        .iter()
        .filter_map(|r| r.coverage.as_ref().map(|v| (r.claim.clone(), v.clone())))
        .collect::<Vec<_>>();
    let mut result = super::mapping::audit(raw, input, claims)?;
    let Some(context) = &input.knowledge else {
        return Ok(result);
    };
    let entries = context["receipts"].as_array().cloned().unwrap_or_default();
    let allowed: std::collections::BTreeSet<_> = entries
        .iter()
        .filter_map(|e| e["receipt_id"].as_str())
        .collect();
    let receipts = receipts
        .iter()
        .filter(|r| allowed.contains(r.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut visible = std::collections::BTreeMap::new();
    for entry in &entries {
        let Some(id) = entry["receipt_id"].as_str() else {
            continue;
        };
        if !entry["result"].is_object() {
            continue;
        }
        let visible = visible
            .entry(id.to_owned())
            .or_insert_with(std::collections::BTreeSet::new);
        for hit in entry["result"]["hits"].as_array().into_iter().flatten() {
            if hit["source_excerpt"].as_str().is_none_or(|s| s.is_empty()) {
                continue;
            }
            for declaration in hit["declarations"].as_array().into_iter().flatten() {
                if let Some(id) = declaration["identity"]["statement_id"].as_str() {
                    visible.insert(id.to_owned());
                }
            }
        }
    }
    let mut invalid = 0;
    for reading in &mut result.claims {
        let matching = proposals
            .iter()
            .filter(|(id, _)| *id == reading.claim.as_str())
            .collect::<Vec<_>>();
        let Some(claim) = claims.iter().find(|c| c.id == reading.claim) else {
            continue;
        };
        if matching.len() != 1 {
            invalid += 1;
            continue;
        }
        match comparison(matching[0].1.clone(), claim, &receipts, auditor, &visible) {
            Ok(coverage) => {
                if let Some(conjecture) = &mut reading.conjecture {
                    match coverage.relation {
                        CoverageRelation::Covered => {
                            conjecture.status = ConjectureStatus::KnownTrue;
                            conjecture.status_reason = coverage.summary.clone();
                        }
                        CoverageRelation::Refuted => {
                            conjecture.status = ConjectureStatus::KnownFalse;
                            conjecture.status_reason = coverage.summary.clone();
                        }
                        _ => {}
                    }
                } else if coverage.relation == CoverageRelation::Refuted {
                    reading.correctness = Correctness::Error;
                    reading.shape = Some(ProofShape::Undetermined);
                    reading.witnesses.clear();
                    if let Some(method) = &mut reading.methodology {
                        method.escape = ProofShape::Undetermined;
                        method.witnesses.clear();
                    }
                }
                reading.coverage = Some(coverage);
            }
            Err(()) => invalid += 1,
        }
    }
    if invalid > 0 {
        result.summary.push_str(&format!(" {invalid} statement(s) had no usable source-based knowledge comparison; no coverage or novelty qualification is inferred from those omitted outputs."));
    }
    Ok(result)
}
