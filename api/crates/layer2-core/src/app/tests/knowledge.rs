use super::*;

#[tokio::test]
async fn knowledge_receipts_are_durable_private_idempotent_and_version_bound() {
    let w = World::new().await;
    let author = w.person("author", &[]).await;
    let outsider = w.person("outsider", &[]).await;
    let reviewer = w.person("reviewer", &[Role::Reviewer]).await;
    let paper = w.submit(&author, "t", false).await;
    let paper = w.review_paper(&author, &paper).await;
    let round = w
        .app
        .begin_referee_round(&reviewer, &paper.id)
        .await
        .unwrap();
    assert!(round.knowledge_events.is_empty());
    let subject = w
        .app
        .knowledge_subject(&reviewer, &paper.id, 1, &paper.claims[0].id)
        .await
        .unwrap();
    let mut privileged_author = author.clone();
    privileged_author.roles.insert(Role::Reviewer);
    assert!(matches!(
        w.app
            .knowledge_subject(&privileged_author, &paper.id, 1, &paper.claims[0].id)
            .await,
        Err(CoreError::Forbidden(_))
    ));
    let query = "the confirmed mathematical statement".to_owned();
    let scopes = vec![
        "configured source lookup".to_owned(),
        KNOWLEDGE_RANKER_SCOPE.into(),
    ];
    let now = chrono::Utc::now();
    let intent = KnowledgeQueryIntent {
        id: "query-1".into(),
        subject: subject.clone(),
        snapshot: None,
        index_revision: None,
        request_digest: KnowledgeQueryReceipt::request_digest(
            &subject, &None, &None, &query, &scopes,
        ),
        query: query.clone(),
        scopes: scopes.clone(),
        admitted_at: now,
    };
    let receipt = KnowledgeQueryReceipt {
        id: intent.id.clone(),
        protocol: KNOWLEDGE_QUERY_PROTOCOL.into(),
        subject,
        snapshot: None,
        index_revision: None,
        request_digest: intent.request_digest.clone(),
        query,
        scopes,
        state: KnowledgeQueryState::Unavailable,
        response: None,
        returned: vec![],
        limits: vec!["test source lookup was unavailable".into()],
        retryable: true,
        started_at: now,
        completed_at: now,
    };
    let completed = KnowledgeEvent::QueryCompleted {
        receipt: receipt.clone(),
    };
    assert!(
        w.app
            .record_knowledge_event(&reviewer, &paper.id, completed.clone())
            .await
            .is_err()
    );
    let admitted = KnowledgeEvent::QueryAdmitted { intent };
    assert!(matches!(
        w.app
            .record_knowledge_event(&author, &paper.id, admitted.clone())
            .await,
        Err(CoreError::Forbidden(_))
    ));
    w.app
        .record_knowledge_event(&reviewer, &paper.id, admitted)
        .await
        .unwrap();
    let first = w
        .app
        .record_knowledge_event(&reviewer, &paper.id, completed.clone())
        .await
        .unwrap();
    let replay = w
        .app
        .record_knowledge_event(&reviewer, &paper.id, completed)
        .await
        .unwrap();
    assert_eq!(first, replay);
    assert_eq!(first.knowledge_events.len(), 2);
    let mut changed_receipt = receipt.clone();
    changed_receipt.limits = vec!["different outcome".into()];
    assert!(
        w.app
            .record_knowledge_event(
                &reviewer,
                &paper.id,
                KnowledgeEvent::QueryCompleted {
                    receipt: changed_receipt
                }
            )
            .await
            .is_err()
    );
    let view = w.app.referee(&author, &paper.id).await.unwrap();
    assert_eq!(view.rounds[0].knowledge_events.len(), 2);
    assert!(matches!(
        w.app.referee(&outsider, &paper.id).await,
        Err(CoreError::NotFound { .. })
    ));

    let mut changed = paper.clone();
    changed.claims_revision += 1;
    w.app.save(&mut changed).await.unwrap();
    let mut late = receipt;
    late.id = "late-query".into();
    assert!(
        w.app
            .record_knowledge_event(
                &reviewer,
                &paper.id,
                KnowledgeEvent::QueryCompleted { receipt: late }
            )
            .await
            .is_err()
    );
    let next = w
        .app
        .begin_referee_round(&reviewer, &paper.id)
        .await
        .unwrap();
    assert_eq!(next.rounds.len(), 2);
    assert_eq!(next.knowledge_events, first.knowledge_events);
    let view = w.app.referee(&author, &paper.id).await.unwrap();
    assert!(view.rounds[1].knowledge_events.is_empty());
    assert_eq!(view.rounds[0].knowledge_events.len(), 2);
}

#[tokio::test]
async fn a_review_pins_one_index_and_rejects_a_second_snapshot() {
    use sha2::{Digest, Sha256};
    let w = World::new().await;
    let author = w.person("author", &[]).await;
    let reviewer = w.person("reviewer", &[Role::Reviewer]).await;
    let paper = w.submit(&author, "t", false).await;
    let paper = w.review_paper(&author, &paper).await;
    w.app
        .begin_referee_round(&reviewer, &paper.id)
        .await
        .unwrap();
    let subject = w
        .app
        .knowledge_subject(&reviewer, &paper.id, 1, &paper.claims[0].id)
        .await
        .unwrap();
    let mut intent = KnowledgeQueryIntent {
        id: "first".into(),
        subject,
        snapshot: Some(format!("sha256:{:x}", Sha256::digest(b"snapshot"))),
        index_revision: Some(format!("sha256:{:x}", Sha256::digest(b"index"))),
        request_digest: String::new(),
        query: "a statement".into(),
        scopes: vec!["statement index".into(), KNOWLEDGE_RANKER_SCOPE.into()],
        admitted_at: chrono::Utc::now(),
    };
    intent.request_digest = KnowledgeQueryReceipt::request_digest(
        &intent.subject,
        &intent.snapshot,
        &intent.index_revision,
        &intent.query,
        &intent.scopes,
    );
    w.app
        .record_knowledge_event(
            &reviewer,
            &paper.id,
            KnowledgeEvent::QueryAdmitted {
                intent: intent.clone(),
            },
        )
        .await
        .unwrap();
    intent.id = "second".into();
    intent.index_revision = Some(format!("sha256:{:x}", Sha256::digest(b"updated index")));
    intent.request_digest = KnowledgeQueryReceipt::request_digest(
        &intent.subject,
        &intent.snapshot,
        &intent.index_revision,
        &intent.query,
        &intent.scopes,
    );
    assert!(
        w.app
            .record_knowledge_event(
                &reviewer,
                &paper.id,
                KnowledgeEvent::QueryAdmitted { intent }
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn completed_query_requires_the_actual_immutable_response_bytes() {
    use sha2::{Digest, Sha256};
    let w = World::new().await;
    let author = w.person("author", &[]).await;
    let reviewer = w.person("reviewer", &[Role::Reviewer]).await;
    let paper = w.submit(&author, "t", false).await;
    let paper = w.review_paper(&author, &paper).await;
    w.app
        .begin_referee_round(&reviewer, &paper.id)
        .await
        .unwrap();
    let subject = w
        .app
        .knowledge_subject(&reviewer, &paper.id, 1, &paper.claims[0].id)
        .await
        .unwrap();
    let mut intent = KnowledgeQueryIntent {
        id: "actual-query".into(),
        subject,
        snapshot: Some(format!("sha256:{:x}", Sha256::digest(b"snapshot"))),
        index_revision: Some(format!("sha256:{:x}", Sha256::digest(b"index"))),
        request_digest: String::new(),
        query: "statement".into(),
        scopes: vec![
            "bounded source search".into(),
            KNOWLEDGE_RANKER_SCOPE.into(),
        ],
        admitted_at: chrono::Utc::now(),
    };
    intent.request_digest = KnowledgeQueryReceipt::request_digest(
        &intent.subject,
        &intent.snapshot,
        &intent.index_revision,
        &intent.query,
        &intent.scopes,
    );
    w.app
        .record_knowledge_event(
            &reviewer,
            &paper.id,
            KnowledgeEvent::QueryAdmitted {
                intent: intent.clone(),
            },
        )
        .await
        .unwrap();
    let blob = w
        .app
        .ports
        .blobs
        .put(b"{\"hits\":[]}", "application/json")
        .await
        .unwrap();
    let mut receipt = KnowledgeQueryReceipt {
        id: intent.id,
        protocol: KNOWLEDGE_QUERY_PROTOCOL.into(),
        subject: intent.subject,
        snapshot: intent.snapshot,
        index_revision: intent.index_revision,
        request_digest: intent.request_digest,
        query: intent.query,
        scopes: intent.scopes,
        state: KnowledgeQueryState::Complete,
        response: Some(blob.clone()),
        returned: vec![],
        limits: vec![],
        retryable: false,
        started_at: intent.admitted_at,
        completed_at: chrono::Utc::now(),
    };
    receipt.response.as_mut().unwrap().id = "not-stored".into();
    assert!(
        w.app
            .record_knowledge_event(
                &reviewer,
                &paper.id,
                KnowledgeEvent::QueryCompleted {
                    receipt: receipt.clone()
                }
            )
            .await
            .is_err()
    );
    receipt.response = Some(blob.clone());
    receipt.response.as_mut().unwrap().sha256 = "0".repeat(64);
    assert!(
        w.app
            .record_knowledge_event(
                &reviewer,
                &paper.id,
                KnowledgeEvent::QueryCompleted {
                    receipt: receipt.clone()
                }
            )
            .await
            .is_err()
    );
    receipt.response = Some(blob);
    w.app
        .record_knowledge_event(
            &reviewer,
            &paper.id,
            KnowledgeEvent::QueryCompleted { receipt },
        )
        .await
        .unwrap();
}

// Core service fixtures supply real stored synthetic query bytes and append
// both admission and completion. Retrieval itself is exercised by worker tests.
pub(super) async fn with_comparisons(
    w: &World,
    reviewer: &Caller,
    paper: &Submission,
    mut audit: RefereeAudit,
) -> RefereeAudit {
    use sha2::{Digest, Sha256};
    let hash = |s: &[u8]| format!("sha256:{:x}", Sha256::digest(s));
    for reading in &mut audit.claims {
        let subject = w
            .app
            .knowledge_subject(reviewer, &paper.id, 1, &reading.claim)
            .await
            .unwrap();
        let query = paper.claim(&reading.claim).unwrap().statement.clone();
        let scopes = vec![
            KNOWLEDGE_RANKER_SCOPE.into(),
            "explicit synthetic source fixture".to_owned(),
            KNOWLEDGE_ORACLE_SCOPE.into(),
        ];
        let snapshot = Some(hash(b"test-publication"));
        let index_revision = Some(hash(b"test-index"));
        let intent = KnowledgeQueryIntent {
            id: format!("fixture:{}:{}", paper.id, reading.claim),
            request_digest: KnowledgeQueryReceipt::request_digest(
                &subject,
                &snapshot,
                &index_revision,
                &query,
                &scopes,
            ),
            subject: subject.clone(),
            snapshot: snapshot.clone(),
            index_revision: index_revision.clone(),
            query: query.clone(),
            scopes: scopes.clone(),
            admitted_at: w.app.ports.clock.now(),
        };
        w.app
            .record_knowledge_event(
                reviewer,
                &paper.id,
                KnowledgeEvent::QueryAdmitted {
                    intent: intent.clone(),
                },
            )
            .await
            .unwrap();
        let response = w
            .app
            .ports
            .blobs
            .put(
                br#"{"fixture":"empty bounded source search","hits":[]}"#,
                "application/json",
            )
            .await
            .unwrap();
        let receipt = KnowledgeQueryReceipt {
            id: intent.id.clone(),
            protocol: KNOWLEDGE_QUERY_PROTOCOL.into(),
            subject: subject.clone(),
            snapshot,
            index_revision,
            request_digest: intent.request_digest,
            query,
            scopes,
            state: KnowledgeQueryState::Complete,
            response: Some(response),
            returned: vec![],
            limits: vec!["Synthetic fixture only.".into()],
            retryable: false,
            started_at: intent.admitted_at,
            completed_at: w.app.ports.clock.now(),
        };
        w.app
            .record_knowledge_event(
                reviewer,
                &paper.id,
                KnowledgeEvent::QueryCompleted { receipt },
            )
            .await
            .unwrap();
        reading.coverage = Some(ClaimCoverage {
            subject,
            receipt_ids: vec![intent.id],
            relation: CoverageRelation::Unknown,
            qualification: CoverageQualification::SourceAudited {
                auditor: reviewer.person.clone(),
                audit_id: format!(
                    "referee-audit:{}:r1:v{}:c{}",
                    paper.id,
                    paper.current_version().unwrap().number,
                    paper.claims_revision
                ),
            },
            binding: None,
            summary: "No matching source in this test fixture.".into(),
            limits: vec!["Not a claim of global novelty.".into()],
        });
    }
    audit
}

#[tokio::test]
async fn a_claimed_certificate_cannot_replace_an_independent_source_audit_in_admission() {
    let w = World::new().await;
    let author = w.person("author", &[]).await;
    let reviewer = w.person("reviewer", &[Role::Reviewer]).await;
    let paper = w.submit(&author, "t", false).await;
    let paper = w.review_paper(&author, &paper).await;
    w.app
        .begin_referee_round(&reviewer, &paper.id)
        .await
        .unwrap();
    let mut audit = with_comparisons(
        &w,
        &reviewer,
        &paper,
        audit_for(
            &paper,
            Recommendation::Reject,
            Correctness::Gap,
            ProofShape::Undetermined,
            None,
        ),
    )
    .await;
    let (file, _) = w.app.referee_file(&paper.id).await.unwrap();
    let round = file.current().unwrap();
    assert!(
        crate::app::knowledge::knowledge_admission(&file, round, &audit.claims[0]).sufficient()
    );
    for protocol in [
        "ordinary Lean verifier",
        "stratalint.truth-export.v2",
        "unrecognized.coverage.v1",
    ] {
        audit.claims[0].coverage.as_mut().unwrap().qualification =
            CoverageQualification::CertificateClaim {
                protocol: protocol.into(),
                artifact_digest: format!("sha256:{}", "a".repeat(64)),
            };
        let admission = crate::app::knowledge::knowledge_admission(&file, round, &audit.claims[0]);
        assert!(admission.complete, "the real query receipt remains intact");
        assert!(!admission.sufficient());
        assert_eq!(admission.audited_relation, None);
    }
}
