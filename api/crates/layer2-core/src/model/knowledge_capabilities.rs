//! Capabilities actually admitted by the downstream consumer. These are not
//! publication-specific proof receipts or claims about every upstream service.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct KnowledgeCapabilities {
    pub source_query_protocol: &'static str,
    pub default_source_ranker: &'static str,
    pub coverage_certificates: CoverageCertificateSupport,
}
#[derive(Debug, Clone, Serialize)]
pub struct CoverageCertificateSupport {
    pub supported: bool,
    pub recognized_protocols: Vec<String>,
    pub reason_code: &'static str,
    pub reason: &'static str,
    pub required_bindings: Vec<&'static str>,
    pub ordinary_lean_establishes_coverage: bool,
}
impl KnowledgeCapabilities {
    pub fn current() -> Self {
        Self {
            source_query_protocol: super::KNOWLEDGE_QUERY_PROTOCOL,
            default_source_ranker: super::KNOWLEDGE_RANKER_SCOPE,
            coverage_certificates: CoverageCertificateSupport {
                supported: false,
                recognized_protocols: vec![],
                reason_code: "no_admitted_coverage_checker",
                reason: "The admitted publication and source-index contracts provide source evidence, not a recognized certificate for entailment of a Mathesis target from its exact prerequisite set. Source-audited comparisons retain their actual evidence level.",
                required_bindings: vec![
                    "confirmed_target_digest",
                    "joint_prerequisite_identities",
                    "snapshot_and_index_revision",
                    "binding_operations",
                    "checker_protocol_and_version",
                    "toolchain",
                    "verified_artifact_digest",
                    "verification_authority",
                ],
                ordinary_lean_establishes_coverage: false,
            },
        }
    }
}
