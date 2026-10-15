use super::*;
use crate::ports::{JobKind, LeasedJob};
async fn fixture() -> (
    World,
    Caller,
    Caller,
    Submission,
    PublicBindingBoard,
    NewBindingContribution,
) {
    let (w, a, r, mut p, _) = research::fixture().await;
    // Fixture publication state; this is not live editorial or author acceptance.
    p.status = SubmissionStatus::Accepted {
        record: "WP-2026-0001".into(),
    };
    p.analysis_visibility = Visibility::Public;
    p.open_to_contributors = true;
    w.app.save(&mut p).await.unwrap();
    let view = w.app.private_bindings(&a, &p.id).await.unwrap();
    let o = &view.options[0];
    let new=NewBindingBoard{option_digest:o.digest.clone(),sources:o.target.sources.clone(),context:"This public mathematical context is explicitly approved; private manuscript details are withheld.".into(),license:BINDING_LICENSE.into(),consent:BINDING_SHARING_CONSENT.into()};
    let b = w
        .app
        .publish_binding_board(&a, &p.id, new.clone())
        .await
        .unwrap();
    assert_eq!(
        b.id,
        w.app
            .publish_binding_board(&a, &p.id, new)
            .await
            .unwrap()
            .id
    );
    let c = NewBindingContribution {
        binding: KnowledgeBinding {
            premises: b.target.sources.clone(),
            relation: BindingRelation::Implies,
            parameters: vec![ParameterBinding {
                parameter: "x".into(),
                expression: "n + 1".into(),
            }],
            hypotheses: vec![HypothesisDischarge {
                hypothesis: "n is positive".into(),
                argument: "Follows from the stated domain".into(),
            }],
            argument: "Apply the selected source after this substitution.".into(),
            unresolved: vec![],
            source_lineage: vec![],
        },
        license: BINDING_LICENSE.into(),
        consent: BINDING_CONTRIBUTION_CONSENT.into(),
    };
    (w, a, r, p, b, c)
}
fn job(p: &Submission) -> LeasedJob {
    LeasedJob {
        submission: p.id.clone(),
        kind: JobKind::BindingAudit,
        attempts: 1,
        lease: "fixture".into(),
    }
}
fn inspection(p: &PendingBindingAudit, verdict: BindingVerdict) -> BindingInspection {
    BindingInspection {
        verdict,
        reason: "PRIVATE audit reasoning must not be public".into(),
        evidence: "PRIVATE manuscript inspection: fixture only, not mathematical acceptance".into(),
        inspected_sources: p.contribution.binding.premises.clone(),
        evidence_complete: true,
    }
}
async fn audit(w: &World, r: &Caller, p: &Submission, verdict: BindingVerdict) {
    let next = w
        .app
        .pending_binding_audit(r, &p.id)
        .await
        .unwrap()
        .unwrap();
    w.app
        .record_binding_inspection(
            r,
            &job(p),
            &TestAgentLease(true),
            &next,
            inspection(&next, verdict),
        )
        .await
        .unwrap();
}
#[tokio::test]
async fn explicit_public_summary_is_author_only_current_and_revocable() {
    let (w, a, r, p, b, c) = fixture().await;
    let outsider = w.person("outsider", &[]).await;
    assert!(w.app.private_bindings(&outsider, &p.id).await.is_err());
    let view = w.app.private_bindings(&a, &p.id).await.unwrap();
    let option = &view.options[0];
    let new = NewBindingBoard {
        option_digest: option.digest.clone(),
        sources: option.target.sources.clone(),
        context: "new context".into(),
        license: BINDING_LICENSE.into(),
        consent: BINDING_SHARING_CONSENT.into(),
    };
    assert!(
        w.app
            .publish_binding_board(&r, &p.id, new.clone())
            .await
            .is_err()
    );
    assert!(
        w.app
            .publish_binding_board(
                &a,
                &p.id,
                NewBindingBoard {
                    consent: String::new(),
                    ..new
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        w.app
            .binding_directory(None, None)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let public =
        serde_json::to_string(&w.app.public_binding_board(&p.id, &b.id).await.unwrap()).unwrap();
    assert!(!public.contains("receipt_ids"));
    assert!(!public.contains("knowledge_events"));
    w.app
        .contribute_binding(&outsider, &p.id, &b.id, c.clone())
        .await
        .unwrap();
    let next = w
        .app
        .pending_binding_audit(&r, &p.id)
        .await
        .unwrap()
        .unwrap();
    w.app
        .revoke_binding_board(&a, &p.id, &b.id, "Author revokes sharing".into())
        .await
        .unwrap();
    assert!(w.app.public_binding_board(&p.id, &b.id).await.is_err());
    assert!(
        w.app
            .binding_directory(None, None)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(!w.app.binding_active(&r, &p.id, &b.id).await.unwrap());
    assert!(
        w.app
            .contribute_binding(&outsider, &p.id, &b.id, c)
            .await
            .is_err()
    );
    assert!(
        w.app
            .record_binding_inspection(
                &r,
                &job(&p),
                &TestAgentLease(true),
                &next,
                inspection(&next, BindingVerdict::Supported)
            )
            .await
            .is_err()
    );
    assert_eq!(
        w.app.private_bindings(&a, &p.id).await.unwrap().boards[0]
            .events
            .len(),
        2
    );
}
#[tokio::test]
async fn canonical_credit_challenges_and_appeals_preserve_history_without_private_leaks() {
    let (w, a, r, p, b, c) = fixture().await;
    let mut later_round = b.target.clone();
    later_round.subject.round += 1;
    assert_eq!(c.canonical(&b.target), c.canonical(&later_round));
    let u = w.person("contributor", &[]).await;
    let v = w.person("other", &[]).await;
    let first = w
        .app
        .contribute_binding(&u, &p.id, &b.id, c.clone())
        .await
        .unwrap();
    let id = first.contributions[0].id.clone();
    assert!(first.credits.is_empty());
    assert_eq!(
        w.app
            .contribute_binding(&u, &p.id, &b.id, c.clone())
            .await
            .unwrap()
            .contributions
            .len(),
        1
    );
    audit(&w, &r, &p, BindingVerdict::Supported).await;
    let mut duplicate = c;
    duplicate.binding.argument = "A different explanation of the same binding".into();
    duplicate.binding.parameters[0].expression = "n   + 1".into();
    w.app
        .contribute_binding(&v, &p.id, &b.id, duplicate)
        .await
        .unwrap();
    audit(&w, &r, &p, BindingVerdict::Supported).await;
    let supported = w.app.public_binding_board(&p.id, &b.id).await.unwrap();
    assert_eq!(supported.credits.len(), 1);
    assert_eq!(supported.credits[0].contributor, u.person);
    let public = serde_json::to_string(&supported).unwrap();
    assert!(!public.contains("PRIVATE"));
    assert!(
        serde_json::to_string(&w.app.private_bindings(&a, &p.id).await.unwrap())
            .unwrap()
            .contains("PRIVATE")
    );
    let challenge = NewBindingChallenge {
        contribution: id.clone(),
        evidence: "The premise may fail on boundary inputs".into(),
        license: BINDING_LICENSE.into(),
        consent: BINDING_CONTRIBUTION_CONSENT.into(),
    };
    w.app
        .challenge_binding(&v, &p.id, &b.id, challenge.clone())
        .await
        .unwrap();
    assert_eq!(
        w.app
            .challenge_binding(&v, &p.id, &b.id, challenge)
            .await
            .unwrap()
            .challenges
            .len(),
        1
    );
    assert_eq!(
        w.app
            .public_binding_board(&p.id, &b.id)
            .await
            .unwrap()
            .credits
            .len(),
        1
    );
    audit(&w, &r, &p, BindingVerdict::Rejected).await;
    assert!(
        w.app
            .public_binding_board(&p.id, &b.id)
            .await
            .unwrap()
            .credits
            .is_empty()
    );
    w.app
        .challenge_binding(
            &u,
            &p.id,
            &b.id,
            NewBindingChallenge {
                contribution: id,
                evidence: "Appeal with the explicit boundary calculation".into(),
                license: BINDING_LICENSE.into(),
                consent: BINDING_CONTRIBUTION_CONSENT.into(),
            },
        )
        .await
        .unwrap();
    audit(&w, &r, &p, BindingVerdict::Supported).await;
    let restored = w.app.public_binding_board(&p.id, &b.id).await.unwrap();
    assert_eq!(restored.credits.len(), 1);
    assert_eq!(restored.audits.len(), 4);
    assert_eq!(restored.challenges.len(), 2);
}
#[tokio::test]
async fn binding_audit_requires_real_premise_identity_complete_evidence_and_current_lease() {
    let (w, _, r, p, b, c) = fixture().await;
    let u = w.person("contributor", &[]).await;
    let mut forged = c.clone();
    forged.binding.premises[0].declaration_name_key = "not returned".into();
    assert!(
        w.app
            .contribute_binding(&u, &p.id, &b.id, forged)
            .await
            .is_err()
    );
    let mut unresolved = c.clone();
    unresolved
        .binding
        .unresolved
        .push("A missing bridge".into());
    w.app
        .contribute_binding(&u, &p.id, &b.id, unresolved)
        .await
        .unwrap();
    audit(&w, &r, &p, BindingVerdict::Supported).await;
    let view = w.app.public_binding_board(&p.id, &b.id).await.unwrap();
    assert_eq!(view.audits[0].verdict, BindingVerdict::Unclear);
    assert!(view.credits.is_empty());
    w.app.contribute_binding(&u, &p.id, &b.id, c).await.unwrap();
    let next = w
        .app
        .pending_binding_audit(&r, &p.id)
        .await
        .unwrap()
        .unwrap();
    assert!(
        w.app
            .record_binding_inspection(
                &u,
                &job(&p),
                &TestAgentLease(true),
                &next,
                inspection(&next, BindingVerdict::Supported)
            )
            .await
            .is_err()
    );
    assert!(
        w.app
            .record_binding_inspection(
                &r,
                &job(&p),
                &TestAgentLease(false),
                &next,
                inspection(&next, BindingVerdict::Supported)
            )
            .await
            .is_err()
    );
    let mut incomplete = inspection(&next, BindingVerdict::Supported);
    incomplete.inspected_sources.clear();
    w.app
        .record_binding_inspection(&r, &job(&p), &TestAgentLease(true), &next, incomplete)
        .await
        .unwrap();
    assert!(
        w.app
            .public_binding_board(&p.id, &b.id)
            .await
            .unwrap()
            .credits
            .is_empty()
    );
    let mut changed = w.app.load(&p.id).await.unwrap();
    changed.claims_revision += 1;
    w.app.save(&mut changed).await.unwrap();
    assert!(w.app.public_binding_board(&p.id, &b.id).await.is_err());
    assert!(!w.app.binding_active(&r, &p.id, &b.id).await.unwrap());
}
#[tokio::test]
async fn binding_submissions_are_fenced_under_concurrency_and_rate_limited() {
    let (w, _, _, p, b, c) = fixture().await;
    let u = w.person("contributor", &[]).await;
    let (a, z) = tokio::join!(
        w.app.contribute_binding(&u, &p.id, &b.id, c.clone()),
        w.app.contribute_binding(&u, &p.id, &b.id, c.clone())
    );
    a.unwrap();
    z.unwrap();
    for n in 1..5 {
        let mut x = c.clone();
        x.binding.argument = format!("Distinct argument {n}");
        w.app.contribute_binding(&u, &p.id, &b.id, x).await.unwrap();
    }
    let mut extra = c;
    extra.binding.argument = "Sixth contribution".into();
    assert!(
        w.app
            .contribute_binding(&u, &p.id, &b.id, extra)
            .await
            .is_err()
    );
    assert_eq!(
        w.app
            .public_binding_board(&p.id, &b.id)
            .await
            .unwrap()
            .contributions
            .len(),
        5
    );
}

#[tokio::test]
async fn publication_contributor_access_and_author_independence_are_separate_gates() {
    let (w, a, r, mut p, b, c) = fixture().await;
    assert!(w.app.contribute_binding(&a, &p.id, &b.id, c).await.is_err());
    for (status, visibility, open) in [
        (SubmissionStatus::InReview, Visibility::Public, true),
        (p.status.clone(), Visibility::Private, true),
        (p.status.clone(), Visibility::Public, false),
    ] {
        p.status = status;
        p.analysis_visibility = visibility;
        p.open_to_contributors = open;
        w.app.save(&mut p).await.unwrap();
        assert!(
            w.app
                .private_bindings(&a, &p.id)
                .await
                .unwrap()
                .options
                .is_empty()
        );
        assert!(w.app.public_binding_board(&p.id, &b.id).await.is_err());
        assert!(!w.app.binding_active(&r, &p.id, &b.id).await.unwrap());
    }
}

#[tokio::test]
async fn first_supported_binding_keeps_priority_after_an_earlier_rejected_contribution_is_appealed()
{
    let (w, _, r, p, b, c) = fixture().await;
    let first = w.person("first", &[]).await;
    let second = w.person("second", &[]).await;
    let view = w
        .app
        .contribute_binding(&first, &p.id, &b.id, c.clone())
        .await
        .unwrap();
    let first_id = view.contributions[0].id.clone();
    audit(&w, &r, &p, BindingVerdict::Rejected).await;
    w.app
        .contribute_binding(&second, &p.id, &b.id, c)
        .await
        .unwrap();
    audit(&w, &r, &p, BindingVerdict::Supported).await;
    w.app
        .challenge_binding(
            &first,
            &p.id,
            &b.id,
            NewBindingChallenge {
                contribution: first_id,
                evidence: "Independent appeal evidence".into(),
                license: BINDING_LICENSE.into(),
                consent: BINDING_CONTRIBUTION_CONSENT.into(),
            },
        )
        .await
        .unwrap();
    audit(&w, &r, &p, BindingVerdict::Supported).await;
    let view = w.app.public_binding_board(&p.id, &b.id).await.unwrap();
    assert_eq!(view.credits.len(), 1);
    assert_eq!(view.credits[0].contributor, second.person);
    let profile = w.app.binding_profile(&second.person, None).await.unwrap();
    assert_eq!(profile.items.len(), 1);
    assert!(profile.items[0].credited);
}
