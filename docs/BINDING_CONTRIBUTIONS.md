# Binding contributions and challenges — implementation contract

P3-A adds a separate consented public summary and L2 contribution path. The
underlying manuscript and private review remain author/staff resources. Existing
publication visibility or the general volunteer toggle does not authorize the
release of private audit/query text.

Only accepted public publication submissions with contributor access enabled can
open a public discussion. Private and unaccepted manuscripts remain private;
no public-summary opt-in bypasses that boundary.

The submitting author approves the exact current confirmed statement, a bounded
public mathematical context, selected declaration identities from its actual
saved source-query receipts, and a public-summary license. Store the complete
source/version/claims-revision/round and exact approval identity. The interface
shows the complete public payload before authorization. Withdrawn/revised inputs
or a revoked summary close new public access and work; historical private events
remain intact. No model creates public consent.

A signed-in contributor submits a structured joint binding with explicit license
and consent: actual premise references, relation, parameter substitution,
hypothesis discharge, argument and remaining obligations. Premises must be in the
author-approved set on the pinned publication. Unknown premises require a new
source inquiry/approval; they are not silently treated as recorded evidence.
Submission is idempotent, bounded and immutable. A Mongo revision fence reserves
at most five contribution/challenge requests per person across all summaries in
a rolling 24 hours; an uncertain admission conservatively retains its slot.
Authors and linked co-authors cannot submit independent binding discoveries on
their own papers. They may provide challenge/correction evidence. A candidate is not
a proof receipt, and no contributor can file their own successful audit.

A durable review job independently checks the binding against the original source
and saved actual query evidence using the existing managed inspector. The
system retains the inspected declaration identities and evidence limits; unread
or truncated necessary material cannot support a confirmed binding. A separate
source-audited receipt records supported/rejected/unclear with the reason and
reproducible evidence, under revision/lease/authorization fences. It does not
rewrite the original audit or publication. Public readback is bounded and avoids
private source/review text; private authors/staff retain complete receipts.

Challenges and contributor appeals append their exact evidence and receive their
own independent inspection. An upheld challenge can revoke support/credit;
a later independently supported appeal can restore it. Historical decisions
remain visible with their current effective status. Revocation of public sharing
closes new access and new auditing; live managed work checks that fence too.

Credit is for one independently supported canonical binding on one pinned target,
not for token count, elapsed challenge time or ordinary proof success. Only
supported equivalent/instance/implies coverage earns binding-discovery credit;
proof-use and refutation inspections retain their separate meaning and history.
The first successful independent inspection determines priority, not the first
unreviewed submission. Canonical
identity retains the complete joint premise set, relation, substitutions and
hypotheses; harmless list-order/whitespace differences do not create a second
credit. Duplicate arguments can remain as attributed submissions but do not
multiply the same binding's credit. Formalized/Solved credit keeps its separate
existing verifier-only path. Do not claim real external participation from test
fixtures or the venue's own activity.

REST/OpenAPI, private author controls, public contribution/challenge readback and
CLI/MCP must project the same Layer 2 operations. Mongo/referee GridFS history is
authoritative; source and consent changes are fences rather than deletions.
Public-summary authorization and contributor licensing do not independently
authorize publishing an upstream proposal; that is the separate P3-B export path.

## Service and evidence boundary

- `GET /bindings` is a paginated directory backed by small metadata stored
  atomically with the checksummed referee history. Each read checks the live
  publication, source, contributor toggle and summary authorization.
- `GET /bindings/{submission}/{board}` exposes only the exact approved target,
  context, source identities, licensed contributor inputs, controlled audit
  verdicts and current credits. Complete independent inspection reason/evidence
  remain private even when a reviewer inspected a private part of the manuscript.
- `GET /binding-contributors/{person}` returns current public contributions and
  credits with discussion/contribution identities; it supports pagination and
  never includes a revoked/private source. Historical events are retained.
- `GET/POST /submissions/{id}/bindings` reads private options/history or lets the
  submitter authorize a summary. `/{board}/revoke` closes sharing and queues
  cancellation. Separate authorization records preserve exact source and query
  identities; the current original publication must remain public.
- `POST /bindings/{id}/{board}/contributions` accepts a structured joint binding.
  `.../challenges` accepts an independently rechecked challenge or appeal. No
  public or contributor endpoint accepts an audit result.
- The review worker uses the operator's independent CMA account, never an
  author's compute binding. It reuses a durable request identity. Live run
  activity, new CMA admission, inspection persistence and cleanup all check the
  current source/consent fence. Original managed runs are retained through waits.
- Inspection receives the saved required source excerpts after their checksums
  and declaration identities are checked, up to 800,000 encoded bytes, plus up
  to 400,000 bytes of the binding and its relevant challenge/audit history.
  Missing/truncated necessary evidence settles as unclear. The source inspector
  must independently report missing definitions, hypotheses and dependencies.
  These are L2 source judgments, never recognized checker certificates.
- At most ten summary histories per submission and 120 discussion events before
  new contribution admission. Pending inspections and revocation can still be
  appended after the intake bound. One current summary per source/claim target
  prevents a later review round from creating simultaneous duplicate credit.
- Canonical identity retains source/target, complete selected declaration set,
  relation, substitutions and hypothesis names. Argument prose, harmless order
  and whitespace changes do not create extra credit. This does not claim full
  mathematical equivalence detection.
- Revocation blocks new serving and work. Previously delivered CC BY 4.0 material
  remains governed by its license. This consent grants no upstream submission
  authority. Source-query text, manuscript text and independent private audit
  evidence are not included in a public license grant.

CLI/MCP tools: `bindings`, `binding`, `binding-profile`, `private-bindings`,
`publish-binding`, `revoke-binding`, `contribute-binding`, `challenge-binding`.
The JSON request shapes are defined in OpenAPI. Public tools return the same
projection as the website. The author's JSON/Markdown review dossier also
includes the private, append-only binding history.

Any real mathematical acceptance requires an actual independent CMA inspection
and actual external contributor participation. Fixtures establish engineering
invariants only. A closed challenge window or an empty discussion never proves
novelty, creates a coverage certificate or modifies a publication decision.
