use super::*;
use crate::knowledge::DeclarationIdentity;

fn publication() -> PublicationIdentity {
    PublicationIdentity {
        bundle_digest: sha256(b"admitted publication"),
        source_commit: "a".repeat(40),
        source_tree: "b".repeat(40),
    }
}

fn declaration(path: &str, name: &str, source: &str) -> PublishedDeclaration {
    PublishedDeclaration {
        identity: DeclarationIdentity {
            snapshot: publication().bundle_digest,
            module_path: path.into(),
            name_key: name.into(),
            statement_id: sha256(format!("{path}:{name}").as_bytes()),
            type_sha256: sha256(name.as_bytes()),
        },
        name: name.into(),
        kind: "theorem".into(),
        frozen_node_id: sha256(path.as_bytes()),
        freeze_status: "frozen".into(),
        source_sha256: sha256(source.as_bytes()),
        axioms: vec![],
        module_imports: vec![],
        prerequisite_frozen_node_ids: vec![],
    }
}

#[test]
fn full_source_content_retrieval_survives_uninformative_declaration_names() {
    let source = "/-- More objects than boxes force a repeated observation. -/\ntheorem x := 0";
    let other = "theorem objects := Nat.zero_le 0";
    let index = SourceIndex::build(
        publication(),
        vec![
            declaration("A.lean", "x", source),
            declaration("B.lean", "objects", other),
        ],
        |path| {
            Ok(if path == "A.lean" { source } else { other }
                .as_bytes()
                .to_vec())
        },
    )
    .unwrap();
    let result = SearchIndex::new(index)
        .unwrap()
        .search("boxes repeated observation", None, 5)
        .unwrap();
    assert_eq!(result.hits.len(), 1);
    assert_eq!(result.hits[0].module_path, "A.lean");
    assert!(result.hits[0].source_excerpt.contains("More objects"));
    assert!(!result.result_truncated);
    assert_eq!(result.searched_modules, 2);
}

#[test]
fn changed_sources_fail_while_missing_and_oversized_sources_are_explicit() {
    let catalog = vec![declaration("A.lean", "a", "correct")];
    assert!(
        SourceIndex::build(publication(), catalog.clone(), |_| Ok(b"changed".to_vec())).is_err()
    );
    let missing =
        SourceIndex::build(publication(), catalog.clone(), |_| Err("missing".into())).unwrap();
    assert_eq!(missing.omissions[0].reason, "source_unavailable");
    let result = SearchIndex::new(missing)
        .unwrap()
        .search("any statement", None, 5)
        .unwrap();
    assert_eq!(result.searched_modules, 0);
    assert_eq!(result.omitted_modules, 1);
    assert!(result.hits.is_empty());
    let huge = SourceIndex::build(publication(), catalog, |_| {
        Ok(vec![b'x'; MAX_SOURCE_BYTES + 1])
    })
    .unwrap();
    assert_eq!(huge.omissions[0].reason, "source_exceeds_limit");
}

#[test]
fn deterministic_revision_binds_sources_catalog_and_omissions() {
    let a = declaration("A.lean", "a", "hello");
    let b = declaration("A.lean", "b", "hello");
    let build = |ds| SourceIndex::build(publication(), ds, |_| Ok(b"hello".to_vec())).unwrap();
    let index = build(vec![a.clone(), b.clone()]);
    assert_eq!(index.revision, build(vec![b, a]).revision);
    let mut corrupt = index.clone();
    corrupt.modules[0].source.push('!');
    assert!(corrupt.validate().is_err());
    let mut corrupt = index;
    corrupt.modules[0].declarations[0].identity.statement_id = "fake".into();
    corrupt.revision = corrupt.content_digest().unwrap();
    assert!(corrupt.validate().is_err());
}

#[test]
fn exact_type_search_retains_the_matching_declaration_under_result_cap() {
    let source = "opaque symbols";
    let declarations = (0..120)
        .map(|n| declaration("A.lean", &format!("d{n:03}"), source))
        .collect::<Vec<_>>();
    let wanted = declarations[119].identity.clone();
    let index = SourceIndex::build(publication(), declarations, |_| {
        Ok(source.as_bytes().to_vec())
    })
    .unwrap();
    let result = SearchIndex::new(index)
        .unwrap()
        .search("unrelated", Some(&wanted.type_sha256), 1)
        .unwrap();
    assert_eq!(result.hits[0].declarations[0].identity, wanted);
    assert_eq!(result.hits[0].declarations.len(), 100);
    assert!(result.hits[0].declarations_truncated);
    assert!(result.result_truncated);
}

#[test]
fn result_limits_bound_utf8_content_and_disclose_clipping() {
    let source = format!("needle {}", "ℝ".repeat(MAX_EXCERPT_BYTES));
    let catalog = (0..8)
        .map(|n| declaration(&format!("A{n}.lean"), "x", &source))
        .collect();
    let index =
        SourceIndex::build(publication(), catalog, |_| Ok(source.as_bytes().to_vec())).unwrap();
    let result = SearchIndex::new(index)
        .unwrap()
        .search("needle real", None, 6)
        .unwrap();
    assert_eq!(result.matching_modules, 8);
    assert_eq!(result.hits.len(), 6);
    assert!(result.result_truncated);
    assert!(
        result
            .hits
            .iter()
            .map(|h| h.source_excerpt.len())
            .sum::<usize>()
            <= MAX_RESULT_SOURCE_BYTES
    );
    assert!(result.hits.iter().all(|h| h.excerpt_truncated));
}

#[test]
fn unsafe_path_and_mixed_snapshot_never_reach_source_loader() {
    for mut d in [
        declaration("../A.lean", "a", "a"),
        declaration("A.lean", "b", "b"),
    ] {
        if d.identity.module_path == "A.lean" {
            d.identity.snapshot = sha256(b"other");
        }
        assert!(
            SourceIndex::build(publication(), vec![d], |_| panic!("must reject before I/O"))
                .is_err()
        );
    }
}

#[test]
fn experimental_identifier_components_preserve_whole_names_and_legacy_behavior() {
    let source = "namespace Project.SemigroupOrbit\ntheorem stabilityWitness := 0\nend Project.SemigroupOrbit";
    let index = SourceIndex::build(
        publication(),
        vec![declaration("A.lean", "stabilityWitness", source)],
        |_| Ok(source.as_bytes().to_vec()),
    )
    .unwrap();
    let legacy = SearchIndex::new(index.clone()).unwrap();
    assert!(
        legacy
            .search("semigroup orbit stability witness", None, 5)
            .unwrap()
            .hits
            .is_empty()
    );
    let candidate = SearchIndex::with_identifier_components(index).unwrap();
    assert_eq!(
        candidate
            .search("semigroup orbit stability witness", None, 5)
            .unwrap()
            .hits
            .len(),
        1
    );
    assert_eq!(
        candidate
            .search("SemigroupOrbit", None, 5)
            .unwrap()
            .hits
            .len(),
        1
    );
    assert_eq!(
        identifier_terms("HTTPResponse"),
        vec!["httpresponse", "http", "response"]
    );
}

#[test]
fn experimental_path_field_discovers_source_without_changing_legacy_or_identity() {
    let source = "theorem result := 0";
    let index = SourceIndex::build(
        publication(),
        vec![declaration("Algebra/SemigroupOrbit.lean", "result", source)],
        |_| Ok(source.as_bytes().to_vec()),
    )
    .unwrap();
    let legacy = SearchIndex::new(index.clone()).unwrap();
    assert!(
        legacy
            .search("semigroup orbit", None, 1)
            .unwrap()
            .hits
            .is_empty()
    );
    let candidate = SearchIndex::with_identifier_fields(index).unwrap();
    assert_eq!(candidate.identity(), legacy.identity());
    let result = candidate.search("semigroup orbit", None, 1).unwrap();
    assert_eq!(result.hits[0].module_path, "Algebra/SemigroupOrbit.lean");
    assert_eq!(result.hits[0].source_excerpt, source);
    assert_eq!(result.hits[0].source_sha256, sha256(source.as_bytes()));
}

#[test]
fn empty_index_still_retains_its_selected_ranker_identity() {
    let empty = SourceIndex::build(publication(), vec![], |_| unreachable!()).unwrap();
    let ranked = SearchIndex::with_identifier_fields(empty.clone()).unwrap();
    assert_eq!(ranked.ranking_protocol(), Some(IDENTIFIER_PATH_RANKER));
    assert_eq!(
        ranked
            .search("anything", None, 6)
            .unwrap()
            .ranking_protocol
            .as_deref(),
        Some(IDENTIFIER_PATH_RANKER)
    );
    let legacy = SearchIndex::new(empty)
        .unwrap()
        .search("anything", None, 6)
        .unwrap();
    assert!(
        serde_json::to_value(legacy)
            .unwrap()
            .get("ranking_protocol")
            .is_none()
    );
}

#[test]
fn readback_uses_full_identity_complete_source_and_an_exact_provenance_line() {
    let marker = format!("Mathesis-Proposal: {}", sha256(b"exact proposal"));
    let source = format!(
        "{}\n-- {marker}\ntheorem exact := 0",
        "prefix ".repeat(MAX_EXCERPT_BYTES)
    );
    let d = declaration("A.lean", "exact", &source);
    let mut changed = d.identity.clone();
    changed.type_sha256 = sha256(b"changed type");
    let index = SearchIndex::new(
        SourceIndex::build(publication(), vec![d.clone()], |_| {
            Ok(source.as_bytes().to_vec())
        })
        .unwrap(),
    )
    .unwrap();
    let (p, found) = index.observe_declarations(&[d.identity.clone(), changed], &marker);
    assert_eq!(p, publication());
    assert_eq!(found[0].0.as_ref().unwrap().identity, d.identity);
    assert!(found[0].1);
    assert!(found[1].0.is_none());
    assert!(!found[1].1);
    let (_, found) = index.observe_declarations(&[d.identity], &format!("{marker}extra"));
    assert!(found[0].0.is_some());
    assert!(!found[0].1);
}
