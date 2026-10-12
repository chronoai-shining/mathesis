use super::*;

#[tokio::test]
async fn archive_search_filters_before_pagination_and_never_reads_private_revisions() {
    let w = World::new().await;
    let author = w.person("author", &[]).await;
    let mut ids = vec![];
    for n in 1..=5 {
        let mut s = w.submit(&author, "t", false).await;
        s.title = format!("Prime gaps {n}");
        s.abstract_text = "UniquePublicationWord".into();
        s.analysis_visibility = if n % 2 == 0 {
            Visibility::Private
        } else {
            Visibility::Public
        };
        let id = RecordId::format(2026, n);
        s.status = SubmissionStatus::Accepted { record: id.clone() };
        w.app.save(&mut s).await.unwrap();
        w.app
            .ports
            .records
            .insert(&Record {
                kind: s.kind,
                id: id.clone(),
                submission: s.id.clone(),
                title: s.title.clone(),
                authors: s.authors.clone(),
                publication: Some(Box::new(s.clone())),
                basis: AdmissionBasis::EscapeWitness,
                accepted_by: author.person.clone(),
                accepted_at: w.app.ports.clock.now(),
            })
            .await
            .unwrap();
        ids.push((id, s.id));
    }
    let query = PaperSearch {
        q: Some("UniquePublicationWord".into()),
        author: Some("Author".into()),
        msc: Some("11".into()),
        kind: Some(SubmissionKind::Paper),
    };
    let first = w
        .app
        .search_papers(query.clone(), Some(2), None)
        .await
        .unwrap();
    assert_eq!(
        first
            .items
            .iter()
            .map(|s| s.record.as_str())
            .collect::<Vec<_>>(),
        vec!["WP-2026-0005", "WP-2026-0003"]
    );
    let second = w
        .app
        .search_papers(query, Some(2), first.next_before)
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].record.as_str(), "WP-2026-0001");
    assert!(second.next_before.is_none());
    let mut revised = w.app.load(&ids[4].1).await.unwrap();
    revised.title = "PrivateRevisionWord".into();
    revised.abstract_text = "PrivateRevisionWord".into();
    w.app.save(&mut revised).await.unwrap();
    assert!(
        w.app
            .search_papers(
                PaperSearch {
                    q: Some("PrivateRevisionWord".into()),
                    ..Default::default()
                },
                None,
                None
            )
            .await
            .unwrap()
            .items
            .is_empty()
    );
    w.app
        .set_analysis_visibility(&author, &ids[4].1, Visibility::Private)
        .await
        .unwrap();
    assert!(
        w.app
            .search_papers(
                PaperSearch {
                    q: Some(ids[4].0.to_string()),
                    ..Default::default()
                },
                None,
                None
            )
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(
        w.app
            .search_papers(
                PaperSearch {
                    q: Some("*".into()),
                    ..Default::default()
                },
                None,
                None
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn portable_review_preserves_provenance_and_author_only_access() {
    let w = World::new().await;
    let author = w.person("author", &[]).await;
    let stranger = w.person("stranger", &[]).await;
    let staff = w.person("reviewer", &[Role::Reviewer]).await;
    let s = w.submit(&author, "t", false).await;
    assert!(matches!(
        w.app.review_report(&stranger, &s.id).await,
        Err(CoreError::NotFound { .. })
    ));
    let report = w.app.review_report(&author, &s.id).await.unwrap();
    assert_eq!(report.version, 1);
    assert_eq!(report.source_sha256, s.versions[0].archive.sha256);
    assert_eq!(report.claims_revision, 0);
    assert_eq!(report.service, "Orchestra");
    let staff_report = w.app.review_report(&staff, &s.id).await.unwrap();
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        serde_json::to_value(staff_report).unwrap()
    );
}
