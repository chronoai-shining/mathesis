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
