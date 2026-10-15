# Existing publication consumption — 16 October 2026

The Mathesis reader consumes an existing Pages source publication and its
original Trureturing inspector report. No upstream code, workflow, artifact,
judge or publication was changed, and no Lean build was run during acquisition.

## Source and trust observations

| Coordinate | Observed value |
| --- | --- |
| Trureturing source | `e7dcc89974fd44cabd0f10781eccc135eddf8d1c` |
| Source tree | `442f387c1013f60d23f70faa1927ea29fda57d21` |
| Pages producer | `f1161cfae0317f3eea9481b26f2dae96e63cd65c` |
| Publication digest | `sha256:6c92665f572be41df6f8fad0d246f4e141e9cf036196608239792f787121ec26` |
| Source dev CI | [38719956711](https://github.com/the-omega-institute/trureturing/actions/runs/38719956711), attempt 1, success, exact source SHA |
| Original report producer | [38727419398](https://github.com/the-omega-institute/trureturing/actions/runs/38727419398), attempt 1, success, exact source SHA |
| Pinned report environment | Lean 4.33.0, Mathlib `db584cd6d46c92f209a44c0f1c829460d327499d` |

GitHub separately confirmed that the source is an ancestor of dev. The report
producer's branch is `integration-ci-fib-isolation-tests`; the exact same source
also has successful dev CI. The branch name and the transport manifest's own
trust booleans were not used alone to establish that observation.

The compact archive is available at the existing
[Pages source release](https://github.com/the-omega-institute/trureturing-pages/releases/tag/pages-source-e7dcc89974fd44cabd0f10781eccc135eddf8d1c).
Its transport schemas are `pages-source-publication.v1` and
`pages-report-projection.v1`, not an upstream truth-release. The adapter checks
the exact SHA256SUMS bytes against the admitted bundle digest, all five artifact
hashes and the agreement of publication, manifest, snapshot and native export.

The original report release tag is
`lean-cache-verify-v1-db584cd6d46c92f209a44c0f1c829460d327499d-linux-arm64-report-stratalint-raw-lean-report-v3-env-43c54620c14f-ci-38727419398-1`.
All five archive parts were individually verified, then the complete archive
SHA-256 was checked:
`e8b26b3e03f61cc5f068c1f21648cd2ff8f683836b51f6f7d4c17aa24fd847dd`.
Only the report and its four sidecars were extracted; archive contents were not
executed.

| Original member | SHA-256 |
| --- | --- |
| `raw-lean-report.json` | `62f6bb798078ea4789d289aeaa0c89f98f685b7df85a0bd90fe9702a1255feb5` |
| `.input.attestation` | `f66bedf142f54faadf80faf9826600d92c3c15fc3037d0fff7859c246834756a` |
| `.materials.zip` | `8d42ab8216ee30ebfd4ee59f190409161104f7ed8905ed9c40e3a42bfa7db478` |
| `.provenance.json` | `28d30d49f0f5cbedc1e2bce6c31d72f9e458f8399723af594af31895adff10cb` |
| `.sha256` | `64039b5d517600b5f056c690dcfc1d3f8fe251c1a77ca1fd50c664f2fb4ab82d` |

The reader also verifies provenance schema v4 and the original input-coordinate
preimages, rather than accepting an arbitrary hashed attestation as well formed.

## Actual byte and identity checks

The native export contains 8,685 modules: 8,580 `frozen` and 105
`proven-not-yet-frozen`. These are index inventory counts, not theorem counts,
field density or a coverage percentage. The raw report has 9,783 modules;
material availability does not itself give an unexported module Frozen status.

The material archive has 353,298 entries, 621,504,574 compressed bytes and
24,094,897,100 expanded bytes. Its largest single entry is 610,982,677 bytes.
The adapter streams the archive hash, validates member names and regularity,
does not extract paths, and caps an individual requested material at 16 MiB.
An oversized selected material fails explicitly. The first successful probe
selected four development families and returned 25 declarations with checked
canonical materials, in 168,959 serialized bytes. It is not a full index or a
completed semantic retrieval service.

The native pigeonhole declaration independently matches:

- module `D5/S0/Diagonal/PigeonholeFiber.lean`;
- statement id `sha256:7055ad403a73e564741999ceb43535300eeb0a5af556401ff847379f29eb8e9e`;
- type material id `sha256:19fb5e929d4eace8def791dd16565af005dff5655ac34ea57f9a35e6b86364d9`.

The material bytes are a `statement-v1(...)` expression. Its plain file SHA-256
is a different value. The consumer's native-identity calculation agrees with
the pinned upstream `materials.py`, including a separate supplementary-Unicode
vector. No kernel run or certificate qualification is inferred from that hash
agreement.

## Reproduction and gates

The `knowledge_probe` example uses the same reader intended for application
composition. Supply the existing extracted projection directory, original
report directory, an independently admitted identity JSON with
`bundle_digest/source_commit/source_tree`, and explicit module paths:

```sh
cd api
cargo run --locked -p mathesis-review --example knowledge_probe -- \
  /path/to/projection /path/to/original-report /path/to/admitted-identity.json \
  D5/S0/Diagonal/PigeonholeFiber.lean \
  D5/S1/Recurrence/CassiniFricke.lean \
  D5/S3/Quantum/Matrix/ProjectionAMGMRefutation.lean \
  D5/S0/Asymptotics/Bonferroni/TailBounds.lean > consumed.json
```

Initial adapter regressions cover the real material vector, Unicode identity,
Closed/Frozen separation, renamed-declaration impersonation, an admitted-source
mismatch despite a positive transport trust flag, corrupt report/sidecar/archive,
path traversal and missing requested modules. Domain regressions cover partial
query plus established relation, fabricated/cross-version receipts, missing
hypotheses, misuse of ordinary Lean as a binding certificate, self-support,
proof-use versus entailment, input integrity and mixed snapshots.

Private query admissions, completed receipts and assessments are appended to the
existing revision-fenced referee history, including its checksummed GridFS
storage. New service regressions establish actual store round trips, idempotent
replay, immutable receipts, author/outsider visibility, changed-claims rejection,
history retention and one pinned index per round. Legacy histories load with an
empty event list, not fabricated receipts.

At this checkpoint 309 ordinary API tests passed with real MongoDB and TeX;
two environment-dependent real Lean tests were explicitly ignored locally.
Full strict Clippy passed. PR #3's distinct merged head passed its real Linux
Lean/image gates in CI `38783335655`. Neither that earlier CI result nor these
adapter tests establish that the new knowledge pipeline has been deployed.

At that checkpoint the reader/domain foundations alone did not close issue #4.
The subsequent ingestion checkpoint below records the additional completed work.

## Full source catalog and durable import

The full exported declaration catalog was subsequently checked against the
original report and fixed-commit source. It contains **158,749 declaration
records** in 8,679 modules with nonempty exported declarations. Six of the 8,685
exported modules contain no exported declaration, including the root aggregation
module `Trureturing.lean`; they do not become theorem-search candidates.

The bounded source index includes **8,673 modules**, with six source-size
omissions explicitly retained:

- `D5/S3/Arith/Covering/Erdos203ConditionalCapacity.lean`
- `D5/S3/Quantum/Dynamics/CycleUniformMixing/OrbitIncidenceChecker.lean`
- `D5/S3/Quantum/Dynamics/CycleUniformMixing/OrbitIndexedSums.lean`
- `D5/S3/Quantum/Dynamics/CycleUniformMixing/SupportMaskChecker.lean`
- `D5/S3/Quantum/Entanglement/AbsolutePPT/QutritN10PurityBound.lean`
- `D5/S3/Quantum/Entanglement/HiguchiSudbery/PrimeHierarchyCertificate.lean`

The index revision is
`sha256:2838a557bfddda5a1f9410e0dcf596a67112c0ec0f163c4cf1a787abcce15d2b`,
with 306,815,996 serialized bytes. Full-source term search for
`finite reading map observations cardinal pigeonhole collision fiber` returned
the published pigeonhole source in position 2 of 10. This is one diagnostic
query, not measured benchmark recall or a semantic entailment result. The
six held-out evaluation cases were not queried.

The application importer then stored the exact index and 13 original/transport/
license artifacts in task-owned MongoDB/GridFS, totaling 1,255,646,274 bytes,
at `2026-10-15T18:58:41.476427Z`. This is a local application ingestion result,
not a production deployment. The operator admission retains exact source and
report CI URLs and the independently observed source evidence; URL syntax is
not used as evidence that GitHub authenticated those claims.

New index tests cover checked-source integrity, missing/oversized modules,
content retrieval with uninformative declaration names, exact type matches
under declaration truncation, stable revisions, mixed snapshots, traversal and
bounded UTF-8 excerpts. MongoDB tests verify an index larger than the document
limit survives reconnect, exact re-import is immutable, original artifacts stay
referenced by offline GC, changed checksums fail, and oversized or changed
streamed uploads abort without completed file records. Query-domain storage
also requires an actual checksummed response payload before a successful
receipt is admitted.

See `KNOWLEDGE_OPERATIONS.md` for reproduction. Remaining work is the leased
review integration, mathematical comparison, policy, author presentation and
end-to-end evaluation. A functioning source index alone does not close issue #4.

Final foundation gates: **318 ordinary API tests passed** with real MongoDB and
TeX, strict workspace/all-target/all-feature Clippy and cargo-deny passed, and
18 evaluation tests passed. Two real Lean tests remain explicitly ignored in
this local environment and are delegated to the repository's normal Linux CI
verifier/image gates. A first fully concurrent run encountered the existing
2-second CMA CLI stream test timeout during full-snapshot ingestion; after that
job completed, the full workspace passed with two local test threads. No shared
CI workflow or runner setting changed.
