# Mathesis capability delivery

Scope: complete the Mathesis work in P0–P3 of
[the capability plan](MATHESIS_CAPABILITY_PLAN_2026-10-16.md). Consume Trureturing's
existing publications and interfaces; do not modify or publish upstream code,
judge rules, workflows or artifacts. This checklist records completion evidence,
not intentions. The objective remains active until every applicable item is
implemented and verified.

| Item | State | Evidence / remaining work |
| --- | --- | --- |
| P0-A admission and compatibility | Engineering merged; live audit pending | PR #3 merged as `69c1a5776f9adb27c0eab187aa4181cd802815cf`. Head `19b3ec7` passed all applicable checks in CI `38783335655`, including actual Linux Lean/image gates, and CodeQL. Local 291 API, 316 web, 3 SDK tests passed at the integrated head. Complete live contract-3 Oracle/CMA review still needs executable company-managed CMA resources. |
| P0-B evaluation contract and baseline | In progress | 18 source-bound authored cases, 12 development / 6 held out by family; exact-byte freeze, label evidence and scorer implemented. 17 scorer/arithmetic tests pass. See `COVERAGE_EVALUATION.md`. Labels are not yet human-adjudicated. Measured statement-only Oracle baseline: 11/12 usable responses, covered recall 5/6, refutation recall 3/6, one false-open/content reading; raw evidence retained. Holdout unqueried. Scorer now has 18 tests. Full audit and human adjudication remain. |
| P1-A existing artifact consumption | Complete | Downloaded Pages source publication `6c92665f572be41df6f8fad0d246f4e141e9cf036196608239792f787121ec26`; verified artifact bytes and source identities locally. Its Trureturing source is `e7dcc89974fd44cabd0f10781eccc135eddf8d1c`, confirmed ancestor of dev; source CI `38719956711` and report producer `38727419398` both succeeded at that SHA. Original report archive and all five sidecars downloaded and hash-verified; canonical materials use type identity with the upstream hash domain, not plain SHA-256 or declaration identity. Application importer and immutable Mongo/GridFS storage implemented and locally exercised on the full catalog: 158,749 declaration records, 8,673 indexed modules, six explicit oversized-source omissions; 1.256 GB retained including original artifacts and license. Merged in PR #5 (`05c9f1aaf8724bce6930e183d93ea17615e192f7`); CI `38791040803` passed all gates, including real Linux Lean/image validation. A Mongo/GridFS cold-load query matched the pre-storage response exactly. |
| P1-B coverage contract and durable models | Complete | `COVERAGE_CONTRACT.md`, orthogonal models and append-only private query/assessment events implemented in revision-fenced referee history. Source-audited qualification cannot substitute an ordinary Lean certificate. Domain and service tests cover fabricated receipts, version/index fences, self-support, incomplete queries, immutable replay and visibility. Merged in PR #5 with 318 passing API tests, strict Clippy and Linux CI/image checks. Success receipts require the exact stored response payload and its checksum; self-review is rejected. Runtime integration follows in P1-C. |
| P1-C actual queries and review integration | In progress | PR #6 merged as `cfc332cccecc40e9ecb0686f367b34e515ecf33a`; CI `38798106776` passed all gates including real Linux Lean. It performs actual per-claim source queries before Oracle, persists intent/response under revision and lease fences, and supplies the same bounded saved evidence to Oracle/CMA/advice/letter. Snapshot changes, lease loss and cancellation retain admitted identity. Structured CMA comparisons resolve only saved receipt/declaration ids. PR #7 merged as `a7afc249731a5cd6214ed83706b993b18dd5e445`; CI `38801787654` passed all gates, including real Linux Lean. Contract 2 adds atomic Oracle reformulations and one recorded CMA follow-up batch. Local 338 API and 320 Web tests passed. Knowledge contract 3 adds explicitly pinned identifier/path ranking and term-focused reformulations; pending integration/CI is tracked in the retrieval worktree. End-to-end mathematical quality remains P1-E. |
| P1-D policy and author reports | In progress | PR #6 implements the application-derived gate: it derives admission from actual current receipts and the audit. Missing required evidence leaves publication pending while private advice/letters continue; correctness errors remain prior blockers and named settlement stays separate. Private JSON/Markdown/UI show query limits, bindings and unresolved obligations; a checksummed source-response download has matching SDK/CLI/MCP access. PR #7 adds explicit missing-semantic-query status in Markdown/UI/model context, with compatibility and negative regressions. Engineering checks passed; live quality remains P1-E. |
| P1-E end-to-end quality | In progress | All 12 frozen development inputs queried the real full index through Mongo/GridFS. Raw lexical queries retrieved none of the predeclared complete source sets in the first 6/10 candidates; this is exact-source recall, not a judgment on alternative results. Release cold-load mean 10,353 ms. PR #8 merged as `2c4686adef4ac25c17df49be2a4575dc101d9293`; CI `38805979806` passed all gates including real Linux Lean. All 12 source-informed Oracle calls completed; the 11 cases shared with the old baseline have identical normalized judgments, including the false-open projection case. Actual full-field reformulation recall is 3/12. New pinned identifier/path ranking plus standard terms achieves 10/12 exact-source-set recall at 6/10, confirmed through 12 Mongo/GridFS calls whose responses match direct-index evaluation. See `COVERAGE_EVALUATION.md`. Real CMA audit, full-manuscript/human adjudication and holdout evaluation remain absent. |
| P2-A research obligations and solving | Pending | Authorized precise tasks, existing target/attempt/verifier path, result writeback and reuse. |
| P2-B computational evidence replay | Engineering validated; PR #9 integration checks pending | Exact rational three-projection AM–GM replay reproduces −8/243. 61 independent Python contraction checks agree with Rust. Source/version-bound receipts precede Oracle and feed audit/advice/letter plus private author exports. 343 API and 322 Web tests passed; CI/merge pending. See `COMPUTATION_REPLAY_VALIDATION_2026-10-16.md`. |
| P2-C coverage certificate consumer | Pending | Consume recognized upstream certificates when available, strict target/snapshot/protocol checks; unavailable capability must remain explicit and cannot be simulated by ordinary Lean verification. |
| P3-A binding contributions and challenges | Pending | Independent submissions, audit, appeal, credit/revocation, author consent, API/SDK/MCP/UI. |
| P3-B authorized feedback and consumption | Pending | Proposal packages with provenance/license, existing upstream admission references, verified downstream readback; no automatic upstream write without the applicable explicit authorization. |

Current worktrees:

- P0 integration: `/Users/lexa/Desktop/lexa/omega/mathesis-p0-20261016`, branch
  `codex/mathesis-p0-validation-20261016`; pushes update existing PR #3's branch.
- Knowledge/evaluation: `/Users/lexa/Desktop/lexa/omega/mathesis-knowledge-20261016`,
  branch `codex/mathesis-knowledge-consumption`, based on the P0 candidate.
- Review integration: `/Users/lexa/Desktop/lexa/omega/mathesis-knowledge-review-20261016`,
  branch `codex/mathesis-knowledge-review`.
- Semantic queries: `/Users/lexa/Desktop/lexa/omega/mathesis-semantic-queries-20261016`,
  branch `codex/mathesis-semantic-queries`.
- Finite computation: `/Users/lexa/Desktop/lexa/omega/mathesis-computation-evidence-20261016`,
  branch `codex/mathesis-computation-evidence`.
- Evaluation: `/Users/lexa/Desktop/lexa/omega/mathesis-knowledge-evaluation-20261016`,
  branch `codex/mathesis-knowledge-evaluation`.
- Retrieval ranking: `/Users/lexa/Desktop/lexa/omega/mathesis-retrieval-ranking-20261016`,
  branch `codex/mathesis-retrieval-ranking`.
- Research obligations preparation: `/Users/lexa/Desktop/lexa/omega/mathesis-research-obligations-20261016`,
  branch `codex/mathesis-research-obligations`.
- Original analysis: `/Users/lexa/Desktop/lexa/omega/mathesis-capability-plan-20261016`.

Operational observations: local tests use a task-owned MongoDB on port 27031 and
an isolated Rust target directory `/private/tmp/mathesis-p0-target`. Do not stop
the unrelated MongoDB on port 27029 or edit the main checkout's concurrent web
changes. P0 red/green and verification logs use `/private/tmp/mathesis-p0-*`.
Source acquisition evidence lives in `/private/tmp/mathesis-capability-evidence-20261016`.

Current live resource observations: the historical Mathesis CMA workspace is
ready, but its sandbox `sbx_0bd035c17ae9469f8e412cf52cb3f2b1` is `retired`;
the other visible sandbox is `retired`. Oracle has ready workers. No live CMA
execution is claimed from workspace status alone, and no shared profile or
upstream workspace has been changed. Completed statement-only Oracle acquisition uses
`/private/tmp/mathesis-evaluation-oracle-baseline` and retains every streaming
handle and original response. It does not count as a full source-audited review.

The Pages bundle is explicitly `pages-report-projection.v1`, not an upstream
truth-release. Its provenance binds the original raw-report sidecars and exact
native exporters. The referenced report was published from an integration
producer, but its exact source commit independently has successful dev CI and
is reachable from dev; neither the transport tag nor its self-asserted trust
fields alone establish this fact.

Latest GitHub observation: issue #4 was closed on 2026-10-15 at 19:24:16 UTC
by `AlyciaBHZ`, immediately after PR #5 merged. This checklist retains its
unfulfilled acceptance work; GitHub closure is not evidence of runtime,
semantic-retrieval or end-to-end completion.
