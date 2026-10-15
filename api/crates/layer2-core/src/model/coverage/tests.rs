use super::*;

fn example() -> (ClaimCoverage, Vec<KnowledgeQueryReceipt>) {
    let subject = CoverageSubject {
        submission: "paper".into(),
        version: 1,
        claims_revision: 1,
        round: 1,
        claim: "C1".into(),
        source_digest: digest(b"source"),
        claim_digest: digest(b"forall x, P(x)"),
    };
    let reference = KnowledgeDeclarationRef {
        snapshot: digest(b"snapshot"),
        module_path: "D5/Example.lean".into(),
        declaration_name_key: "ns(n0,7:example)".into(),
        statement_id: digest(b"declaration"),
        type_digest: digest(b"type"),
    };
    let at = "2026-10-16T00:00:00Z".parse().unwrap();
    let mut receipt = KnowledgeQueryReceipt {
        id: "query-1".into(),
        protocol: KNOWLEDGE_QUERY_PROTOCOL.into(),
        subject: subject.clone(),
        snapshot: Some(reference.snapshot.clone()),
        index_revision: Some(digest(b"index")),
        request_digest: String::new(),
        query: "forall x, P(x)".into(),
        scopes: vec!["admitted publication, statement and source index".into()],
        state: KnowledgeQueryState::Partial,
        response: Some(crate::model::BlobRef {
            id: "stored-fixture-output".into(),
            bytes: 2,
            sha256: digest(b"{}")[7..].into(),
        }),
        returned: vec![reference.clone()],
        limits: vec!["one oversized declaration omitted".into()],
        retryable: true,
        started_at: at,
        completed_at: at,
    };
    receipt.request_digest = KnowledgeQueryReceipt::request_digest(
        &receipt.subject,
        &receipt.snapshot,
        &receipt.index_revision,
        &receipt.query,
        &receipt.scopes,
    );
    let coverage = ClaimCoverage {
        subject,
        receipt_ids: vec![receipt.id.clone()],
        relation: CoverageRelation::Covered,
        qualification: CoverageQualification::SourceAudited {
            auditor: "auditor".into(),
            audit_id: "audit-1".into(),
        },
        binding: Some(KnowledgeBinding {
            premises: vec![reference],
            relation: BindingRelation::Instance,
            parameters: vec![ParameterBinding {
                parameter: "x".into(),
                expression: "x".into(),
            }],
            hypotheses: vec![HypothesisDischarge {
                hypothesis: "x is in X".into(),
                argument: "confirmed domain is X".into(),
            }],
            argument: "Instantiate the checked source with the confirmed parameters.".into(),
            unresolved: vec![],
            source_lineage: vec![],
        }),
        summary: "This claim is an instance of the cited result.".into(),
        limits: vec![],
    };
    (coverage, vec![receipt])
}

#[test]
fn partial_query_and_established_coverage_coexist() {
    let (coverage, receipts) = example();
    coverage.validate(&receipts).unwrap();
    assert!(!coverage.required_query_complete(&receipts));
    let mut complete = receipts;
    complete[0].state = KnowledgeQueryState::Complete;
    assert!(coverage.required_query_complete(&complete));
}

#[test]
fn fabricated_or_cross_version_receipts_are_rejected() {
    let (mut coverage, receipts) = example();
    coverage.receipt_ids = vec!["model-invented".into()];
    assert!(coverage.validate(&receipts).is_err());
    coverage.receipt_ids = vec!["query-1".into()];
    coverage.subject.version = 2;
    assert!(coverage.validate(&receipts).is_err());
}

#[test]
fn unresolved_hypothesis_is_not_an_entailment() {
    let (mut coverage, receipts) = example();
    coverage.binding.as_mut().unwrap().hypotheses[0]
        .argument
        .clear();
    assert!(coverage.validate(&receipts).is_err());
    coverage.relation = CoverageRelation::Candidate;
    coverage.qualification = CoverageQualification::Candidate;
    coverage.validate(&receipts).unwrap();
}

#[test]
fn source_audit_and_binding_certificate_are_not_interchangeable() {
    let (mut coverage, receipts) = example();
    coverage.qualification = CoverageQualification::CertificateClaim {
        protocol: "ordinary Lean verifier".into(),
        artifact_digest: digest(b"kernel success"),
    };
    assert!(coverage.validate(&receipts).is_err());
    coverage.relation = CoverageRelation::Candidate;
    coverage.validate(&receipts).unwrap();
}

#[test]
fn manuscript_derivatives_cannot_be_their_own_authority_baseline() {
    let (mut coverage, receipts) = example();
    coverage
        .binding
        .as_mut()
        .unwrap()
        .source_lineage
        .push(coverage.subject.submission.clone());
    assert!(coverage.validate(&receipts).is_err());
}

#[test]
fn a_proof_use_edge_does_not_establish_coverage() {
    let (mut coverage, receipts) = example();
    coverage.binding.as_mut().unwrap().relation = BindingRelation::ProofUse;
    assert!(coverage.validate(&receipts).is_err());
}

#[test]
fn request_content_and_result_snapshot_are_bound() {
    let (coverage, mut receipts) = example();
    receipts[0].query = "a changed query".into();
    assert!(coverage.validate(&receipts).is_err());
    let (coverage, mut receipts) = example();
    receipts[0].returned[0].snapshot = digest(b"another snapshot");
    assert!(coverage.validate(&receipts).is_err());
}

#[test]
fn comparison_cannot_mix_old_and_new_snapshot_observations() {
    let (mut coverage, mut receipts) = example();
    let mut next = receipts[0].clone();
    next.id = "query-2".into();
    next.snapshot = Some(digest(b"next snapshot"));
    next.returned.clear();
    next.request_digest = KnowledgeQueryReceipt::request_digest(
        &next.subject,
        &next.snapshot,
        &next.index_revision,
        &next.query,
        &next.scopes,
    );
    coverage.receipt_ids.push(next.id.clone());
    receipts.push(next);
    assert!(coverage.validate(&receipts).is_err());
}
