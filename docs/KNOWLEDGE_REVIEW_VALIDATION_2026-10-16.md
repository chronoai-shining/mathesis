# Required source-query review integration — 2026-10-16

This increment extends the publication consumer merged in PR #5. New rounds run
actual bounded queries before Oracle, then retain and reuse the same source
responses for independent audit, advice and letter. It does not claim a completed
semantic search system or a live end-to-end CMA evaluation.

The operator selects an already imported immutable index through
`WISHPOOL_KNOWLEDGE_INDEX`. Round/claim/source identities, index pin and request
intent are persisted before querying; exact bounded responses are checksummed
and stored before either reviewer sees them. Query results identify candidate
sources and declarations, not mathematical entailment. Failed queries retain
separate timeout, unavailable or invalid-publication states. A replay reuses the
same response and prompt; a lost worker lease cannot file its late result.

The audit maps only returned declaration ids and actual current-round receipt
ids. Bindings retain joint premises, parameter substitutions, hypotheses and
unclosed obligations. Omitted prompt sources cannot establish coverage. A
refuted proved claim loses positive proof witnesses and becomes an error;
covered/refuted conjectures receive matching known-true/known-false readings.
These are source-audited reviewer judgments, never formal coverage certificates.

Layer 2 derives the publication gate from the saved evidence and completed
audit, clearing model-supplied gate values in the binary mapper. Missing queries
or usable comparisons leave publication pending while advice and a private letter
complete. Proof errors still block publication; a covered conclusion cannot
support the new-result route, and named settlement retains its separate audited
route. A restored source permits staff to restart the same manuscript version in
a new round. Old rounds, model identities and evidence remain unchanged.

Authors/staff can inspect the query and comparison in the existing private view,
JSON dossier and readable Markdown. A new private response download returns exact
checksummed JSON with no-store headers; SDK, CLI and MCP expose the same operation.
Other users receive 404, including readers of public papers. Review-only inputs
remain private and cannot acquire a publication decision or Lean probe.

## Local evidence

- Full API suite: **332 passed**, using actual MongoDB at the task-owned port
  27031 and TeX Live. Two real Lean tests remain explicitly ignored locally;
  normal Linux CI/image gates execute them. No upstream Lean was run.
- Strict API Clippy, formatting and cargo-deny passed. The full-suite log is
  `/private/tmp/mathesis-knowledge-review-api-final.log`; final strict-Clippy
  log is `/private/tmp/mathesis-knowledge-review-clippy-final3.log`.
- Web: **319 passed**, formatting, lint, types, production/showcase builds and
  high-severity npm audit passed (zero vulnerabilities).
- Contributor SDK: **3 passed**, formatting and strict Clippy passed.
- Meaningful cases cover real source-index queries through the leased worker,
  shared Oracle/CMA inputs, uncertain Oracle admission, index changes, lease
  loss, interrupted index loading, idempotent completion, forged/cross-claim
  ids, unread source excerpts, missing hypotheses, joint premises, ordinary
  proof-use versus coverage, refutation consistency, failure classification,
  same-version recovery, pending/settlement policy, permissions and legacy replay.

Worker/math mapping fixtures are deliberately synthetic. Their passing tests
establish orchestration and evidence invariants; they do not establish a real
mathematical discovery or independently prove an entailment. PR #5's full-publication
import and exactly matching Mongo/GridFS cold-load query are documented separately
in `KNOWLEDGE_CONSUMPTION_VALIDATION_2026-10-16.md`.

## Remaining acceptance work

Current discovery queries use the confirmed claim's text against checked source
terms. Oracle reformulations and durable additional CMA queries remain next,
followed by source-bound known/generalized/missing-assumption comparisons and the
frozen evaluation protocol. The 6 held-out cases remain unqueried. A real CMA
source-audited review is still unverified because no suitable executable managed
sandbox has been established. No production capability deployment is claimed.

P2/P3 tasks, recognized upstream certificate availability, human-adjudicated
quality labels, external contributions and authorized upstream acceptance/readback
remain separately tracked in `CAPABILITY_DELIVERY.md`. Issue #4's closed status
does not satisfy those acceptance conditions.
