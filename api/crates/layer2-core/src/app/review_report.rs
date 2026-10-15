//! Portable review dossier: source binding plus the existing safe author projection.
use super::App;
use crate::{CoreResult, ids::SubmissionId, model::*};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ReviewReport {
    pub schema_version: u32,
    pub purpose: SubmissionPurpose,
    pub review_scope: String,
    pub service: &'static str,
    pub submission: SubmissionId,
    pub title: String,
    pub authors: Vec<Author>,
    pub version: u32,
    pub claims_revision: u64,
    pub source_sha256: String,
    pub evidence_by_version: std::collections::BTreeMap<u32, Vec<EvidenceAttachment>>,
    pub status: SubmissionStatus,
    pub decision: Option<crate::policy::Decision>,
    pub claims: Vec<Claim>,
    pub review: RefereeView,
    pub research: ResearchView,
    pub knowledge_capabilities: KnowledgeCapabilities,
}
impl App {
    pub async fn review_report(
        &self,
        caller: &Caller,
        id: &SubmissionId,
    ) -> CoreResult<ReviewReport> {
        let submission = self.submission(caller, id).await?;
        let mut file = self
            .ports
            .referees
            .get(id)
            .await?
            .unwrap_or_else(|| RefereeFile::new(id.clone()));
        for round in &mut file.rounds {
            if round.source_sha256.is_empty() {
                round.source_sha256 = submission
                    .versions
                    .iter()
                    .find(|v| v.number == round.version)
                    .map(|v| v.archive.sha256.clone())
                    .unwrap_or_default();
            }
        }
        let version = submission.current_version();
        Ok(ReviewReport {
            schema_version: 3,
            knowledge_capabilities: self.knowledge_capabilities(),
            research: self.research_obligations(caller, id).await?,
            evidence_by_version: submission
                .versions
                .iter()
                .map(|v| (v.number, v.evidence.clone()))
                .collect(),
            purpose: submission.purpose,
            review_scope: submission.review_scope.clone(),
            service: "Orchestra",
            submission: id.clone(),
            title: submission.title.clone(),
            authors: submission.authors.clone(),
            version: version.map_or(0, |v| v.number),
            source_sha256: version.map_or_else(String::new, |v| v.archive.sha256.clone()),
            claims_revision: submission.claims_revision,
            status: submission.status.clone(),
            decision: submission.decision.clone(),
            claims: submission.claims.clone(),
            review: file.view(false),
        })
    }
}

impl ReviewReport {
    /// Portable author-safe text uses exactly the same projection as JSON.
    pub fn markdown(&self) -> String {
        use std::fmt::Write;
        let mut out = format!(
            "# Orchestra review — {}\n\nSubmission: {} · Current version: {}\n\nSource SHA-256: {}\n\nReview focus: {}\n\nModel review is research feedback, not a Lean proof certificate. Each round below identifies its own source.\n",
            self.title, self.submission, self.version, self.source_sha256, self.review_scope
        );
        let capability = &self.knowledge_capabilities.coverage_certificates;
        let _ = writeln!(
            out,
            "\nCoverage certificate support: {}. {}\n",
            capability.supported, capability.reason
        );
        if let Some(crate::policy::Decision::Pending { detail, .. }) = &self.decision {
            let _ = writeln!(out, "Publication decision pending: {detail}\n");
        }
        for (version, files) in &self.evidence_by_version {
            if files.is_empty() {
                continue;
            }
            let _ = writeln!(
                out,
                "\n## Evidence — source version {version}\n\nAuthor-supplied files; availability is not an independent validation receipt.\n"
            );
            for file in files {
                let _ = writeln!(
                    out,
                    "- {} · {} bytes · SHA-256 {} — {}",
                    file.descriptor.path, file.bytes, file.sha256, file.descriptor.description
                );
            }
        }
        for entry in &self.research.obligations {
            let _ = writeln!(
                out,
                "\n## Private research obligation {}\n\nState: {}\n\n{}\n\nCorrespondence: {}\n\nChecked targets are internal evidence only. The original claim requires a fresh independent audit.\n",
                entry.obligation.id,
                entry.state,
                entry.obligation.goal.statement(),
                entry.obligation.goal.correspondence
            );
            if let Some(target) = &entry.target {
                let _ = writeln!(
                    out,
                    "Target SHA-256: {} · toolchain {}\n",
                    target.digest, target.toolchain
                );
            }
            for attempt in &entry.attempts {
                if let Some(receipt) = &attempt.receipt {
                    let _ = writeln!(
                        out,
                        "Attempt {}: {:?} · target {} · solution {} · checked {}\n",
                        attempt.id,
                        receipt.verdict,
                        receipt.target_digest,
                        receipt.solution_digest,
                        receipt.checked_at
                    );
                }
            }
        }
        for round in &self.review.rounds {
            let _ = writeln!(
                out,
                "\n## Round {} — source version {}\n\nSource SHA-256: {}\n",
                round.number, round.version, round.source_sha256
            );
            if let Some(method) = &round.review_method {
                let _ = writeln!(
                    out,
                    "Review method: {method} · contract {} · claims revision {}\n",
                    round.review_contract, round.claims_revision
                );
            }
            for receipt in &round.computation_receipts {
                let _ = writeln!(
                    out,
                    "### Replayed finite computation — {}\n\nState: {:?} · protocol: {}\n\nExact value: {}/{}\n\nInput SHA-256: {} · source version {} · claims revision {}\n\n{}\n",
                    receipt.attachment.descriptor.path,
                    receipt.state,
                    receipt.protocol,
                    receipt.numerator.as_deref().unwrap_or("unavailable"),
                    receipt.denominator.as_deref().unwrap_or("unavailable"),
                    receipt.attachment.sha256,
                    receipt.version,
                    receipt.claims_revision,
                    receipt.explanation
                );
            }
            if round.knowledge_contract > 0 {
                let _ = writeln!(
                    out,
                    "### Required knowledge queries\n\nCompletion covers only the recorded bounded plan, never all mathematical knowledge. No match is not evidence of global novelty.\n"
                );
                if round.knowledge_contract >= 2 {
                    let claims: std::collections::BTreeSet<_> = round
                        .knowledge_events
                        .iter()
                        .map(|event| &event.subject().claim)
                        .collect();
                    for claim in claims {
                        if !round.knowledge_events.iter().any(|event| {
                            matches!(event,
                            KnowledgeEvent::QueryCompleted { receipt }
                                if &receipt.subject.claim == claim
                                    && receipt.scopes.iter().any(|s| s == KNOWLEDGE_ORACLE_SCOPE))
                        }) {
                            let _ = writeln!(
                                out,
                                "Required semantic query has no recorded result for statement {claim}.\n"
                            );
                        }
                    }
                }
                for event in &round.knowledge_events {
                    if let KnowledgeEvent::QueryCompleted { receipt } = event {
                        let _ = writeln!(
                            out,
                            "#### Query {} — statement {}\n\nState: {:?} · retryable: {}\n\nSnapshot: {}\n\nIndex: {}\n\nRequest digest: {}\n\nQuery: {}\n\nStarted: {} · completed: {}\n",
                            receipt.id,
                            receipt.subject.claim,
                            receipt.state,
                            receipt.retryable,
                            receipt.snapshot.as_deref().unwrap_or("unavailable"),
                            receipt.index_revision.as_deref().unwrap_or("unavailable"),
                            receipt.request_digest,
                            receipt.query,
                            receipt.started_at,
                            receipt.completed_at
                        );
                        for scope in &receipt.scopes {
                            let _ = writeln!(out, "Scope: {scope}\n");
                        }
                        for limit in &receipt.limits {
                            let _ = writeln!(out, "Query limitation: {limit}\n");
                        }
                        if let Some(response) = &receipt.response {
                            let _ = writeln!(
                                out,
                                "Saved response SHA-256: {} · {} bytes\n",
                                response.sha256, response.bytes
                            );
                        }
                        for source in &receipt.returned {
                            let _ = writeln!(
                                out,
                                "Returned candidate: {} · {}\n\nStatement identity: {} · type digest: {}\n",
                                source.module_path,
                                source.declaration_name_key,
                                source.statement_id,
                                source.type_digest
                            );
                        }
                    }
                }
            }
            if let Some(report) = round.referee.done() {
                let _ = writeln!(out, "### Referee report\n\n{}\n", report.summary);
                for c in &report.concerns {
                    let _ = writeln!(out, "- {:?}: {}\n", c.severity, c.issue);
                }
                for limit in &report.limits {
                    let _ = writeln!(out, "Unchecked / limitation: {limit}\n");
                }
            }
            if let Some(audit) = round.audit.done() {
                let _ = writeln!(out, "### Source audit\n\n{}\n", audit.summary);
                for c in &audit.claims {
                    let _ = writeln!(
                        out,
                        "#### Statement {} — {:?}\n\n{}\n",
                        c.claim, c.correctness, c.comment
                    );
                    if let Some(known) = &c.known {
                        let _ = writeln!(out, "Reported prior work: {known}\n");
                    }
                    for witness in &c.witnesses {
                        let _ = writeln!(out, "Audited witness: {witness}\n");
                    }
                    if let Some(coverage) = &c.coverage {
                        let _ = writeln!(
                            out,
                            "Knowledge relation: {:?} · qualification: {:?}\n\n{}\n\nReceipts: {}\n",
                            coverage.relation,
                            coverage.qualification,
                            coverage.summary,
                            coverage.receipt_ids.join(", ")
                        );
                        if let Some(binding) = &coverage.binding {
                            let _ = writeln!(
                                out,
                                "Joint premise relation: {:?}\n\n{}\n",
                                binding.relation, binding.argument
                            );
                            for premise in &binding.premises {
                                let _ = writeln!(
                                    out,
                                    "Joint premise: {} · {} · {}\n",
                                    premise.module_path,
                                    premise.declaration_name_key,
                                    premise.statement_id
                                );
                            }
                            for parameter in &binding.parameters {
                                let _ = writeln!(
                                    out,
                                    "Parameter: {} → {}\n",
                                    parameter.parameter, parameter.expression
                                );
                            }
                            for hypothesis in &binding.hypotheses {
                                let _ = writeln!(
                                    out,
                                    "Required hypothesis: {}\n\nDischarge: {}\n",
                                    hypothesis.hypothesis,
                                    if hypothesis.argument.trim().is_empty() {
                                        "Unresolved"
                                    } else {
                                        &hypothesis.argument
                                    }
                                );
                            }
                            for obligation in &binding.unresolved {
                                let _ = writeln!(out, "Unresolved obligation: {obligation}\n");
                            }
                        }
                        for limit in &coverage.limits {
                            let _ = writeln!(out, "Comparison limitation: {limit}\n");
                        }
                    }
                    if let Some(method) = &c.methodology {
                        let _ = writeln!(
                            out,
                            "Escape: {:?} · Literature: {:?} · Utility: {:?} ({:?})\n\n{}\n\nAdditional content: {}\n\nValue: {}\n",
                            method.escape,
                            method.literature,
                            method.utility.status,
                            method.utility.kind,
                            method.binding_analysis,
                            method.delta,
                            method.utility.rationale
                        );
                        for prerequisite in &method.baseline {
                            let _ = writeln!(
                                out,
                                "Prerequisite: {} — {}\n\nReported source: {}\n",
                                prerequisite.statement,
                                prerequisite.location,
                                prerequisite.source.as_deref().unwrap_or("manuscript")
                            );
                        }
                        for scope in &method.searched {
                            let _ = writeln!(out, "Search scope: {scope}\n");
                        }
                        for witness in &method.witnesses {
                            let _ = writeln!(
                                out,
                                "Witness ({:?}): {}\n\nLocation: {}\n\nNon-binding step: {}\n\nLive use: {}\n",
                                witness.kind,
                                witness.statement,
                                witness.location,
                                witness.non_binding_step,
                                witness.live_use
                            );
                        }
                        let _ = writeln!(
                            out,
                            "Question/application: {}\n\nProgress: {}\n\nExact premises: {}\n\nDischarge: {}\n",
                            method.utility.problem,
                            method.utility.progress,
                            method.utility.prerequisites,
                            method.utility.discharge
                        );
                        if let Some(settlement) = &method.settlement {
                            let _ = writeln!(
                                out,
                                "Settlement ({:?}): {}\n\nOriginal statement: {}\n\nAssumptions: {}\n\nPrior status: {}\n\nResolution: {}\n\nMatching argument: {}\n\nReported opened source: {}\n",
                                settlement.status,
                                settlement.problem.name,
                                settlement.statement,
                                settlement.assumptions,
                                settlement.prior_status,
                                settlement.resolution,
                                settlement.matching_argument,
                                settlement.opened_source
                            );
                        }
                        if let Some(gain) = &method.information_gain {
                            let _ = writeln!(
                                out,
                                "Separate information-gain analysis ({:?}; not an admission gate)\n\nModel: {}\n\nReadout: {}\n\nTask: {}\n\nCorrespondence: {}\n\nReported evidence: {}\n",
                                gain.status,
                                gain.model,
                                gain.readout,
                                gain.task,
                                gain.correspondence,
                                gain.evidence
                            );
                        }
                        for limit in &method.limits {
                            let _ = writeln!(out, "Methodology limitation: {limit}\n");
                        }
                    }
                }
                for (i, c) in audit.concerns.iter().enumerate() {
                    let _ = writeln!(
                        out,
                        "#### R{}-A{} — {:?}\n\n{}\n\nReported location: {}\n\n{}\n\nEvidence ({}):\n\n{}\n",
                        round.number,
                        i + 1,
                        c.status,
                        c.concern,
                        c.location,
                        c.note,
                        c.basis,
                        c.evidence
                    );
                    for response in self
                        .review
                        .finding_responses
                        .iter()
                        .filter(|r| r.round == round.number && r.concern == i)
                    {
                        let _ = writeln!(
                            out,
                            "Author response on version {}: {}\n\nResponse ID: {} · claims revision {}\n",
                            response.version, response.body, response.id, response.claims_revision
                        );
                        for checked in self.review.rounds.iter().filter(|r| {
                            r.version == response.version
                                && r.claims_revision == response.claims_revision
                                && r.response_ids.contains(&response.id)
                                && r.audit.done().is_some()
                        }) {
                            let _ = writeln!(
                                out,
                                "Re-reviewed in round {}. Read that audit for its conclusion; this does not automatically mark the original finding resolved.\n",
                                checked.number
                            );
                            if let Some(assessment) = checked.audit.done().and_then(|a| {
                                a.response_assessments
                                    .iter()
                                    .find(|a| a.response_id == response.id)
                            }) {
                                let _ = writeln!(
                                    out,
                                    "Reviewer conclusion on version {}: {:?}\n\n{}\n\nThis is review feedback, not an independent verification receipt.\n",
                                    checked.version, assessment.status, assessment.explanation
                                );
                                for evidence in &assessment.evidence {
                                    let _ = writeln!(
                                        out,
                                        "- Attachment: {} · SHA-256 {}\n",
                                        evidence.path, evidence.sha256
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        for letter in &self.review.letters {
            let _ = writeln!(out, "\n## {}\n\n{}\n", letter.subject, letter.body);
        }
        out
    }
}
