# Mathesis local corpus stress test — 13–14 October 2026

This is a local development evaluation, not a production capacity claim. It uses
real source manuscripts and Lean files, an isolated MongoDB database, and one live
Oracle/Orchestra review. Synthetic search fixtures are explicitly marked and have
never been published to the real archive. No upstream research, CMA, Oracle or
NyxID code was changed.

## Environment and evidence

- Baseline: `7f36be78abbd6c0db7b5ae0f924c2ac7b1b956b8` on `develop`.
- macOS, 16 GiB memory; Chrome headless; development API on `127.0.0.1:8091`,
  worker on `8092`, Vite on `5181`, MongoDB on `27029`.
- Database: `mathesis_stress_20261013`; MongoDB cache bounded to 0.25 GiB.
- Evidence directory: `/private/tmp/mathesis-stress-20261013` (local, not committed).
  It contains corpus hashes, request scripts, raw before/after timings, source
  round-trip results, browser screenshots, verifier requests and receipts, and
  service progress. Research source and private review text are not committed here.
- Timings are wall-clock observations on a shared development machine, including
  concurrent test/build activity. They are one bounded run per version, not a
  statistically controlled benchmark or a test of large production indexes.

## Actual corpus intake

| Material | Format | Result |
|---|---|---|
| Generalized Petersen graph note | 18.9 KB `.tex` | Received; 3 statements extracted; PDF built; one actual private review requested |
| Binary subspace orthogonality manuscript | 293.5 KB ZIP | Received; 5 statements extracted; PDF built |
| Sub-quorum coloring manuscript | 30.1 KB ZIP | Received; 41 statements extracted; local PDF build failed because `cleveref.sty` is absent |
| Klein four-group cellular automata manuscript | 30.5 KB `.tex` | Received; 14 statements extracted; PDF built |
| Campbell recurrence note | 10.5 KB `.tex`, anonymous author field | Initially rejected; after the UI fix, received through the actual browser with explicitly labelled test submitter metadata; 3 statements extracted |
| Nikandish Lean formalization | 16.8 KB source | Private deposit received; exact retry returns the same ID; exact source export matches; another account gets 404 |
| KleinFour Lean project | 79 supporting modules, about 549 KB JSON | Rejected by the 32-module ceiling; a separate single nested module also rejected because names must be flat |

A separate intake race submitted the same real Lean source 32 times at 16 concurrent
clients. All responses returned the same deposit ID with no HTTP errors (maximum
76 ms). However, GridFS retained 16 blobs for the one winning record: 15 uploads
were unreferenced. This was fixed: after a conflict, the application reads the
committed winner and best-effort discards only its own fresh blobs absent from
that winner. An uncertain insert error never triggers cleanup. A repeat of the
same 32-request/16-client test retained exactly one record and one blob, zero
unreferenced copies, no HTTP errors and a maximum of 35 ms. Deterministic regression
tests cover simultaneous main/supporting source writes and both lost commit
responses and replayed conflicts. Cleanup still needs general garbage collection
for process crashes, partial uploads and cleanup-storage outages; those cases are
not claimed solved by this change.

Receiving a Lean file is not verifying it. The Nikandish source imports an
additional local research module that was not included in the single-file deposit.
The deposit correctly remains unverified. No research author identity was inferred
from test uploader accounts. Anonymous-source override metadata in this database
is explicitly test-only attribution.

The missing TeX package is a local environment gap. The deployment image already
installs `texlive-latex-extra`; shared CI and deployment dependencies were not
changed. The UI shows the actual compile error and preserves the source.

## Search correctness and bounded load

The isolated archive contains 1,000 cloned, clearly labelled `[STRESS FIXTURE n]`
records: 900 public and 100 private. Fixture acceptance is synthetic; it provides
no evidence of research review. Search never runs the review pipeline for these
records. Correctness checks used 37 results per page; load used 25 results per page.

All nine cases passed: Petersen (200 public results), orthogonality (250), author
Nikandish (250), MSC prefix 05C (900), hyphenated sub-quorum (200), exact public
record (1), exact private record (0), private sentinel (0), and literal author
`.*` (0). There were no duplicate results or missing expected results across pages.
Two early harness expectations were corrected to reflect which every-tenth clones
were private; the API result was correct. Invalid query/operator/type inputs return
422. A page limit above 100 is clamped by the existing API contract.

The measured defect was that a search result constructed the complete public paper,
reloaded its source record and queried solver state for every closed statement.
A 41-statement manuscript magnified this work. Lists now build their summary
without statement/proof-source materialization and inspect proof receipts only
where an approved, confirmed target can exist. Live publication visibility,
target validity and independently verified proof counts remain checked. A regression
test covers a verified proof becoming invalid after target confirmation is removed.

| Concurrent clients | Requests per version | Before P95, ms | After P95, ms | Before req/s | After req/s | HTTP errors before / after |
|---|---|---|---|---|---|---|
| 1 | 240 | 3995 | 227 | 1.5 | 17.9 | 0 / 0 |
| 8 | 240 | 5946 | 476 | 7.1 | 52.4 | 0 / 0 |
| 24 | 240 | 11969 | 766 | 9.9 | 75.6 | 0 / 0 |

The two measured load runs total 1,440 requests, plus correctness and interface
checks. At 24 clients the P95 improved by about 15.6× and observed throughput by
about 7.6×. This database contains no independently verified proof fixtures, so
proof-heavy archive performance still needs separate measurement.

Current search is internal text/metadata search ordered newest first. This test
does not establish external arXiv ingestion, semantic retrieval, citation graph
search, relevance ranking or full-text mathematical retrieval.

## Browser, API, CLI and MCP

- Real browser checked the archive at 320, 390 and 1440 pixels, plus review progress,
  the 41-statement confirmation page, Lean intake and anonymous-source recovery at
  320/1440 pixels. No horizontal page overflow or browser exceptions were observed;
  search pagination changed pages correctly. This is not a complete accessibility audit.
- Fixed the missing author-override entry for uploaded manuscripts. Names are one
  per line, blank preserves source extraction, supplied names replace the source
  list, and entering a name never links an account automatically.
- Parent-path ZIP, source without a main document and PDF-only ZIP all returned 422.
- Author review JSON includes schema 2 and the exact source digest. Unrelated
  accounts get 404 for submission, source, referee, JSON and Markdown review exports.
- The actual contributor CLI searched the database. The stdio MCP server negotiated,
  listed 20 tools, ran a filtered search, exported exact Lean source, and refused
  another author's private review. These tests used local development identities;
  they do not establish production NyxID OIDC/BYOK/delegation behavior.
- Remaining UX observations: TeX math in paper titles is still displayed as raw
  `$...$`; a 41-statement confirmation form is long and would benefit from navigation,
  main-result filtering and clearer completion progress. The progress screen does
  not distinguish a provider queue from active reading precisely enough.

## Live review and independent verification

The original preview was API-only, with no Oracle and no background worker. A
separate test worker is configured to use the existing NyxID operator login for
Oracle and CMA, with a published, fixed CMA agent profile. This changes the test
runtime only; it does not establish a production deployment or author BYOK.

Independent checker result: a real `BooleanStreamDiagonal.lean` from the local
research corpus was rejected unchanged for its `namespace` construct. A copied
version with namespace wrappers removed and its Mathlib submodule import replaced
by `import Mathlib` preserved the exact proposition and proof steps. It proved the
same diagonal/non-enumerability target in 343.8 seconds. The receipt pins:

- Lean `leanprover/lean4:v4.33.0`.
- Mathlib `db584cd6d46c92f209a44c0f1c829460d327499d`.
- Target digest `133245785c04be1f2d61bacc398fbcf7198874296b805927551509448382128b`.
- Project digest `2711eacd16fa3b4f542bfd443d18f0cd5af0def58498f69c981494fe88f10e1e`.
- Axioms: `propext`, `Classical.choice`, `Quot.sound` only.

This is a direct invocation of the actual independent checker, not an accepted
publication, author-confirmed public target, uploaded-project compatibility claim,
or a new leaderboard credit. Only scratch copies were compiled. No upstream Lean
repository was modified.

The negative check reused that exact target but submitted a Lean theorem of type
`True`. Although that theorem compiles, the independent `Check.lean` module rejected
it with a type mismatch against `wishpool_target_prop`, in 221.4 seconds. Thus this
run tested both a real proof and a compilable proof of the wrong proposition.

The real Oracle task completed in 23 minutes 36 seconds, using the provider's
reported `observed_model_switcher=gpt_6`, `observed_model_effort=pro`. The original
handle was retained throughout; no replacement review was submitted. The private
report was saved against the exact manuscript hash and contains evidence-specific
findings. These are model-reported review findings, not independent Lean verification.

CMA audit admission then failed in the real service, before an agent was created:
HTTP 409, `connections_required`, with required services `ornn-api` and
`chrono-llm-public`. Replaying the original request with its exact original bytes
and idempotency key reproduced this response. The published create-agent schema
requires `agent_profile` and does not accept a caller-supplied connections field;
Mathesis supplied the pinned profile. NyxID's service list nevertheless reports
both services connected with no missing credential, while their connection status
is null. This is a concrete connection/readiness discrepancy at the CMA/NyxID
boundary. It was not bypassed by granting permissions, modifying upstream code,
changing shared profiles or replacing the user's existing sandbox.

The existing unrelated workspace agent remained untouched; the test created no
CMA agent. No successful CMA audit, deep reasoning completion, final review letter,
publication decision or leaderboard credit is claimed. The selected published CMA
profile says `gpt-5.6-sol` / `x_high`; no model execution occurred, so this is a
configuration observation only.

The test worker was enabled only in the isolated local environment. The original
API-only preview and production configuration have not been silently represented as
working Orchestra deployments. The local review's terminal failure and saved Oracle
report remain inspectable in the author workspace.

## Next priorities supported by this test

1. Resolve the measured CMA/NyxID connection/readiness discrepancy, then rerun the
   same review through audit, advice and letter. Add operator readiness checks before
   accepting expensive work and show actionable failure/progress states. A running
   web API does not imply Oracle, CMA and the Lean checker are enabled.
2. Add recovery for orphan blobs after crashes and storage failures; the ordinary
   simultaneous-retry leak measured here is fixed, but cleanup is best effort.
3. Treat existing research Lean projects as a primary intake: preserve directory
   structure, detect missing imports/dependency pins, and report compatibility before
   a verification request. The present 32 flat-module contract is too narrow for the
   tested corpus. Expanding it requires a separate verifier threat-model and isolation
   change; this test did not weaken the checker to accept arbitrary Lake execution.
4. Improve long statement confirmation and math title rendering. Keep private review,
   archive publication, source receipt and independently checked proof outcomes distinct.
5. Measure larger and proof-heavy indexes, relevance and external-paper discovery.
   The repaired local search path is useful evidence but not an arXiv-scale load result.

## Regression validation

253 backend tests passed with real MongoDB and local TeX integration enabled;
295 frontend tests passed. TypeScript, ESLint, Rust formatting/clippy and the web
production build passed. The ordinary backend run does not execute the optional
real-Lean integration cases without their workspace variable; the separate actual
corpus checker run is reported independently below.

The first search/UI fix passed all GitHub CI and CodeQL jobs at `c4cf0a0`.
Final cleanup-change checks are recorded with the final commit.
