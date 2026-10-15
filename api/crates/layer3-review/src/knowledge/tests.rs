use super::*;
use serde_json::json;
use std::io::{Cursor, Write};

const MATERIAL: &[u8] = include_bytes!("fixtures/pigeonhole.statement");
const PATH: &str = "D5/S0/Diagonal/PigeonholeFiber.lean";
const NAME: &str =
    "ns(ns(ns(ns(ns(n0,2:D5),2:S0),8:Diagonal),15:PigeonholeFiber),24:finite_reading_has_fiber)";
const TYPE: &str = "sha256:19fb5e929d4eace8def791dd16565af005dff5655ac34ea57f9a35e6b86364d9";
const STATEMENT: &str = "sha256:7055ad403a73e564741999ceb43535300eeb0a5af556401ff847379f29eb8e9e";

struct Fixture {
    identity: PublicationIdentity,
    files: BTreeMap<String, Vec<u8>>,
    sidecars: BTreeMap<String, Vec<u8>>,
    archive: Vec<u8>,
}

impl Fixture {
    fn new(status: &str, name_key: &str, zip_name: &str) -> Self {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        archive
            .start_file(zip_name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(MATERIAL).unwrap();
        let archive = archive.finish().unwrap().into_inner();
        let encode = |value: serde_json::Value| serde_json::to_vec(&value).unwrap();
        let report = encode(json!({"schema":"stratalint-raw-lean-report-v3","modules":[{
            "module":"D5.S0.Diagonal.PigeonholeFiber", "source_path":PATH,
            "source_sha256":sha256(b"source"), "imports":["Mathlib.SetTheory.Cardinal.Order"],
            "declarations":[{"name":"finite_reading_has_fiber","name_key":name_key,
                "kind":"theorem","include_in_statement":true,"statement_id":STATEMENT,
                "type_sha256":TYPE,"axioms":["Classical.choice","Quot.sound","propext"]}]
        }]}));
        let report_hash = sha256(&report);
        let producer = "1".repeat(64);
        let resident = "2".repeat(64);
        let sources = "3".repeat(64);
        let config = "4".repeat(64);
        let (input, repository) = input_coordinates(&producer, &resident, &sources, &config);
        let sidecars: BTreeMap<String, Vec<u8>> = BTreeMap::from([
            (
                "raw-lean-report.json.sha256".into(),
                format!("{}  raw-lean-report.json\n", &report_hash[7..]).into_bytes(),
            ),
            (
                "raw-lean-report.json.input.attestation".into(),
                format!("schema=stratalint-lean-report-input-attestation-v1\nrepository_input_sha256={}\nproducer_sha256={}\nreport_sha256={}\n",
                    &repository[7..],producer,&report_hash[7..]).into_bytes(),
            ),
            (
                "raw-lean-report.json.provenance.json".into(),
                encode(json!({
                    "schema":"stratalint-lean-report-provenance-v4", "report_sha256":&report_hash[7..],
                    "side":"candidate","source_side":"candidate","mode":"produced",
                    "input_address":input,"producer_sha256":producer,"repository_inspector_sha256":resident,
                    "lean_sources_sha256":sources,"lean_config_sha256":config,"module_origins":{}
                })),
            ),
        ]);
        let mut hashes = sidecars
            .iter()
            .map(|(k, v)| (k.clone(), sha256(v)))
            .collect::<BTreeMap<_, _>>();
        hashes.insert("raw-lean-report.json".into(), report_hash);
        hashes.insert(
            "raw-lean-report.json.materials.zip".into(),
            sha256(&archive),
        );
        let commit = "e7dcc89974fd44cabd0f10781eccc135eddf8d1c";
        let tree = "442f387c1013f60d23f70faa1927ea29fda57d21";
        let mut files = BTreeMap::from([
            ("raw-lean-report.json".into(), report),
            (
                "report-provenance.v1.json".into(),
                encode(json!({
                    "schema":"pages-report-provenance.v1","source_repo":"the-omega-institute/trureturing",
                    "source_commit":commit,"source_tree":tree,"report_files":hashes
                })),
            ),
            (
                "source-snapshot.v1.json".into(),
                encode(json!({"source_commit":commit,"source_tree":tree})),
            ),
            ("truth-graph.v1.json".into(), b"{}".to_vec()),
            (
                "truth-export.v1.json".into(),
                encode(json!({
                    "schema":"stratalint.truth-export","schema_version":2,"dialect":"stratalint.truth-export.v2",
                    "source_commit":commit,"source_tree":tree,"nodes":[{
                        "repo_path":PATH,"frozen_node_id":sha256(b"node"),"freeze_status":status,
                        "prerequisite_frozen_node_ids":[],"declarations":[{
                            "declaration_name_key":name_key,"kind":"theorem","statement_id":STATEMENT
                        }]
                    }]
                })),
            ),
        ]);
        let sums = files
            .iter()
            .map(|(name, bytes)| format!("{}  {name}\n", &sha256(bytes)[7..]))
            .collect::<String>();
        let digest = sha256(sums.as_bytes());
        let names = [
            ("raw_lean_report", "raw-lean-report.json"),
            ("report_provenance", "report-provenance.v1.json"),
            ("source_snapshot", "source-snapshot.v1.json"),
            ("truth_export", "truth-export.v1.json"),
            ("truth_graph", "truth-graph.v1.json"),
        ];
        let artifacts = names
            .iter()
            .map(|(key, name)| (*key, json!({"file":name,"sha256":sha256(&files[*name])})))
            .collect::<BTreeMap<_, _>>();
        files.insert("SHA256SUMS".into(), sums.into_bytes());
        files.insert("release-manifest.v1.json".into(), encode(json!({
            "schema":"pages-report-projection.v1","sha256sums_digest":digest,"artifacts":artifacts,
            "source":{"source_repo":"the-omega-institute/trureturing","source_commit":commit,"source_tree":tree},
            "trust":{"commit_on_protected_dev":true}
        })));
        files.insert(
            "truth-release-publication.v1.json".into(),
            encode(json!({
                "schema":"pages-source-publication.v1","bundle_ref":digest,"release_digest":digest,
                "source_commit":commit,"source_tree":tree
            })),
        );
        Self {
            identity: PublicationIdentity {
                bundle_digest: digest,
                source_commit: commit.into(),
                source_tree: tree.into(),
            },
            files,
            sidecars,
            archive,
        }
    }

    fn read(&self) -> ReviewResult<ConsumedPublication> {
        consume(
            &self.identity,
            &self.files,
            &self.sidecars,
            Cursor::new(&self.archive),
            &BTreeSet::new(),
        )
    }
}

fn fixture() -> Fixture {
    Fixture::new("frozen", NAME, &TYPE.replacen(':', "/", 1))
}

#[test]
fn native_material_identity_matches_real_published_vector() {
    assert_eq!(
        material_identities(PATH, "theorem", NAME, MATERIAL).unwrap(),
        (TYPE.into(), STATEMENT.into())
    );
}

#[test]
fn supplementary_unicode_matches_pinned_native_protocol() {
    assert_eq!(
        material_identities(
            "D5/𝕏.lean",
            "theorem",
            "ns(n0,2:𝕏)",
            "statement-v1(uparams=[],type=ec(ns(n0,2:𝕏),[]))".as_bytes()
        )
        .unwrap(),
        (
            "sha256:0ba6c5d93382fcf6afa65b125e4cb7ae0effafc6d222d805cb2a2cb9573ddcb6".into(),
            "sha256:685949b53864e8fe5be90ea6be31100108c380e24993f83bd0962f4418bdd77b".into()
        )
    );
}

#[test]
fn closed_but_unfrozen_is_not_promoted_by_consumption() {
    let input = Fixture::new("proven-not-yet-frozen", NAME, &TYPE.replacen(':', "/", 1));
    let output = input.read().unwrap();
    assert_eq!(
        output.declarations[0].declaration.freeze_status,
        "proven-not-yet-frozen"
    );
    assert_eq!(
        output.declarations[0].canonical_statement,
        std::str::from_utf8(MATERIAL).unwrap()
    );
}

#[test]
fn same_type_cannot_impersonate_another_declaration() {
    let input = Fixture::new("frozen", "ns(n0,8:impostor)", &TYPE.replacen(':', "/", 1));
    assert!(
        input
            .read()
            .unwrap_err()
            .to_string()
            .contains("material identity mismatch")
    );
}

#[test]
fn transport_trust_flag_does_not_override_admitted_source() {
    let mut input = fixture();
    input.identity.source_commit = "0".repeat(40);
    assert!(
        input
            .read()
            .unwrap_err()
            .to_string()
            .contains("publication identity mismatch")
    );
}

#[test]
fn corrupt_report_sidecar_and_archive_fail_before_use() {
    let mut input = fixture();
    input
        .files
        .get_mut("raw-lean-report.json")
        .unwrap()
        .push(b' ');
    assert!(
        input
            .read()
            .unwrap_err()
            .to_string()
            .contains("artifact checksum")
    );
    let mut input = fixture();
    input
        .sidecars
        .get_mut("raw-lean-report.json.sha256")
        .unwrap()
        .push(b' ');
    assert!(
        input
            .read()
            .unwrap_err()
            .to_string()
            .contains("sidecar checksum")
    );
    let mut input = fixture();
    input.archive.push(0);
    assert!(
        input
            .read()
            .unwrap_err()
            .to_string()
            .contains("archive checksum")
    );
}

#[test]
fn archive_paths_are_not_extracted_or_interpreted_as_sources() {
    let input = Fixture::new("frozen", NAME, "../../Target.lean");
    assert!(
        input
            .read()
            .unwrap_err()
            .to_string()
            .contains("materials entry")
    );
}

#[test]
fn source_selection_is_explicit_and_missing_modules_are_errors() {
    let input = fixture();
    let modules = BTreeSet::from(["D5/Missing.lean".into()]);
    assert!(
        consume(
            &input.identity,
            &input.files,
            &input.sidecars,
            Cursor::new(&input.archive),
            &modules
        )
        .unwrap_err()
        .to_string()
        .contains("requested module absent")
    );
}
