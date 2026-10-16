# Authorized feedback validation — 2026-10-16

Scope: Mathesis downstream P3-B, based on P3-A merge
`167f13227929152f9257e237c796910f8665fe0b`. No upstream write, author consent,
external participation, upstream admission/freeze or production rollout is
performed or implied by these engineering checks.

## Implemented behavior

Six proposal channels consume current public Mathesis evidence: independently
admitted open problem, verifier-passed solution/refutation, supported joint
binding, public content witness, four-slot DTR proposal and confirmed erratum.
The submitting author previews exact source/claims, attribution, requested route,
rights holders, original available licenses and verification scope. Unknown
manuscript license is explicitly unrecorded. The current inspected Trureturing
license is Apache-2.0; prior CC-BY-4.0 discussion rights are never silently upgraded.

Preparation stores an immutable checksummed GridFS payload with no authorization
event. Each required owner must separately assert applicable rights and authorize
the exact payload digest. Staff cannot grant someone else's rights. Current
source/evidence and all signatures gate download. A later profile name or relative
proof-credit change is excluded from the immutable verified-solution payload.
Witness proposals contain public statement fields, never another owner's proof.

Revocation and evidence/source changes stop new serving, exports and readback.
Private history and already issued licenses persist. Exact prepares, approvals,
download issuance and observation requests are idempotent. A Mongo revision fence
protects append-only events; the existing offline collector finds payload and
observation references inside large GridFS referee snapshots.

The separate durable readback job uses the existing review lane without models or
payer admission. It reads fixed public GitHub endpoints and an already admitted
immutable index. PR metadata, merge ancestry with exact consumed commit/tree,
complete declaration identity, original source/freeze metadata and optional exact
module provenance are separate facts. Missing inputs and rate limits settle as
saved non-results. No successful result can be submitted through HTTP/CLI/MCP.
The interface has no aggregate upstream acceptance/reimport badge or new credit.

## Checks

- Full local Rust suite: **381 passed**, including real task-owned Mongo/GridFS
  and TeX integration. The two existing actual-Lean cases remain deliberately
  ignored locally; the unchanged Linux verifier-image CI is their gate.
- Full web suite: **331 passed**; strict typecheck/lint/format, app and showcase
  production builds and dependency audit passed.
- SDK: **3 passed**, strict Clippy and format passed. CLI and MCP project every
  preparation, authorization, export, readback and raw-observation operation.
- API strict Clippy, format and cargo-deny passed after boxing the larger history
  event. Dependency duplicates remain the existing permitted warnings.
- Changed source Gitleaks scan passed. No shared CI, runner, release, configuration
  key, upstream output contract or production image tag changed.
- Existing OpenAPI definitions were compared structurally with the P3-A base:
  all previous content is unchanged except the additive private dossier
  `feedback_proposals` field. Eight paths and thirteen schemas are added.

New tests exercise all six channels; exact preview and separate consent; multiple
owners and concurrent authorization; private-data exclusion; invalid/changed
sources and target receipt hashes; unsupported bindings; pending accusation versus
independent rejection; idempotent retries; corrupt/missing payloads; observation
lease loss and revocation; HTTP rejection of client success assertions; actual
Mongo snapshot/GC retention; full-source exact declaration and marker lookup;
GitHub response byte hashes, redirect refusal, oversized/rate-limited bodies,
repository/PR identity, source-tree mismatch and divergent ancestry; durable
worker non-results; and three web consent/projection interactions.

Final focused checks are rerun after the last serialization/projection changes.
PR CI and merge receipts will be recorded in `CAPABILITY_DELIVERY.md`; this local
record does not substitute for those existing gates.

## Acceptance boundaries

Fixture authorizations, verifier/auditor doubles and synthetic upstream metadata
are engineering evidence only. No real authors or external contributors consented
or submitted through this feature during validation. No actual licensed Mathesis
proposal has been admitted upstream and reappeared in a later admitted snapshot.
Module provenance does not prove mathematical correspondence or which PR
introduced the proposal. No recognized coverage-certificate checker is available.

The broader P0–P3 live CMA, full-manuscript/human evaluation, untouched holdout,
external contribution and authorized upstream reimport gates remain recorded in
`CAPABILITY_DELIVERY.md`. Deployment targets and executable company-managed CMA
resources have not been established. Do not turn code/CI completion into a claim
of production availability or measured end-to-end mathematical improvement.
