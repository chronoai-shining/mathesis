# Mathesis knowledge coverage contract

Mathesis consumes immutable Trureturing publications and records its own queries.
An adapter receipt proves what Mathesis queried and received. It is never an
upstream signature, mathematical entailment certificate or universal novelty
claim. The contract below governs downstream storage, reviews and contributions;
upstream judges and publication interfaces remain owned by Trureturing.

## Independent dimensions

Every claim comparison retains these dimensions separately:

| Dimension | Values and meaning |
| --- | --- |
| Query execution | completed within the recorded scope, partial, timed out, unavailable, invalid publication; retryable state and reason are explicit |
| Relation | unknown, candidate, covered, refuted; no-match means no relation was found in the recorded scope |
| Evidence | candidate, independently source-audited, recognized coverage certificate; ordinary Lean success is a separate proof receipt |
| Correctness | existing correct/gap/error/not_checked reading |
| Proof shape | existing content/bind_only/undetermined reading with live witnesses |
| Value | existing utility and supported named-problem settlement readings |

A partial query can still find sufficient evidence for one specific covering
relation. A completed query with no hit does not establish global novelty.
Accepted, Closed and Frozen are different facts. In native export v2 the
consumer preserves `frozen` versus `proven-not-yet-frozen` exactly.

## Identities and provenance

A review subject is `(submission, paper version, claims revision, claim id,
source digest, claim digest, round)`. Local `C1` is not a global theorem id.
A knowledge declaration retains `(snapshot, source repository, source commit,
module path, declaration name key, statement id, type digest)` plus its separate
frozen-node identity and freeze status. Imports and frozen prerequisites remain
module-level information; they are not declaration proof dependencies.

The initial supported transport is `pages-source-publication.v1` with
`pages-report-projection.v1`; it binds the native `stratalint.truth-export` schema
version 2, dialect `stratalint.truth-export.v2`, and original
`stratalint-raw-lean-report-v3` sidecars. It is explicitly a Pages projection,
not an upstream truth-release. Admission records must independently establish
the source revision, successful relevant CI and the immutable bundle digest.
The transport's own `trust` booleans alone cannot establish that admission.

Validate exact artifact bytes, export/report declaration agreement, source hashes,
the original report checksum, attestation, provenance and materials ZIP. Native
statement identity uses `trureturing:statement:v1\0` plus the canonical material;
declaration identity additionally includes module path, declaration kind and name
key in the upstream canonical JSON encoding. Preserve the UTF-16 surrogate
encoding rule for supplementary Unicode. A plain file hash is not that identity.

Canonical materials are serialized expressions, not pretty Lean source. Source
text must be separately retrieved at the pinned commit and checked against the
inspector source hash before display or semantic search. Raw source and model
descriptions never silently replace the canonical identity.

## Queries and indexing

The index records all included modules, excluded or oversized material, input
artifacts, indexer version and supported retrieval operations. An index revision
is immutable. Source/module/declaration counts describe index size, not coverage
density. Domain recall may be reported only with a named finite evaluation set,
sample size and observed result.

Queries retain the original claim, definitions, hypotheses, quantified variables,
conclusion, proposed mathematical search terms and any structured expression.
Retrieve from checked statements and their source text, including mathematical
symbols and hypothesis structure; title/name matching alone is insufficient.
Candidate generation may use model-proposed reformulations. Every executed
reformulation has a receipt under the same snapshot. Similarity is a discovery
signal and never grants a relation qualification.

A receipt contains a stable request id, issuing adapter/protocol, private request
digest, subject, pinned snapshot/index revision, requested and searched scope,
result state, returned declaration identities, completion time and limits. A
failure receipt contains an actual attempted call and sanitized failure reason.
An unconfigured source is explicit absence of capability; do not invent a
successful receipt. Oracle and CMA receive the same admitted receipts. Additional
CMA queries append receipts; model-written receipt ids must resolve to the same
subject and snapshot before they can support a reading.

Knowledge contract 2 requires a completed Oracle reformulation for each assessed
claim in addition to the raw query. Its proposed fields are definitions,
hypotheses, quantifiers, conclusion, search terms and retrieval reason. Query
plans have one origin and at most one query per claim, admitted as one atomic
batch; each origin (Oracle or CMA) has one batch per round. Oracle proposals are
read from its immutable saved report. A crash resumes every admitted unfinished
query before a new audit. CMA follow-up uses a distinct durable audit identity,
with the same payer admission rules and original snapshot. Malformed or
unexecuted additional requests suppress the affected sufficient comparisons;
unassignable gaps affect every comparison. Existing source evidence and
correctness findings are retained. Contracts 0 and 1 preserve their old prompts.

The original archive observed for the first source is approximately 621 MB
compressed and 24 GB expanded, with one material approximately 611 MB. Ingestion
therefore needs streaming hashes, bounded material reads and explicit missing
material states. A small selected-module probe must retain its selection and
must never be presented as a whole-snapshot semantic index.

## Relations and obligations

A relation stores a **set** of premises, the target, parameter substitutions,
every necessary hypothesis and its discharge, the actual argument, sources and
unresolved obligations. A multi-premise argument is one hyperedge; it must not
be rewritten as separate entailments from each premise. Equivalence, instance,
implication, proof use and refutation remain distinct relations.

An independently source-audited L2 comparison can rule out admission as a new
result when all necessary premises and correspondence are established. Pending
hypotheses remain obligations. A changed conclusion, quantifier, necessary
hypothesis or target digest invalidates automatic reuse. A canonical type match
does not alone prove faithfulness to the author's natural-language claim.

No upstream binding-certificate protocol has yet been established for this
integration. Until a recognized protocol and verification authority are
available, evidence remains at its actual level. Kernel success and a tactic
allowlist do not prove bind-only shape. Future certificate consumption must bind
the target, prerequisite set, snapshot, checker/protocol, toolchain and actual
verification artifact, with negative checks for each coordinate.

Ordinary Formalized/Solved credit continues exclusively through the existing
attempt → independent verifier path. Upstream merge, Frozen state, query success,
source-audit agreement and elapsed challenge time cannot substitute for it.

## Persistence, privacy and policy

MongoDB is authoritative; large immutable source/index/report payloads belong in
GridFS. Every mutable aggregate uses revision fencing. Leased jobs retain request
identities across restart; uncertain submissions resume the original work.
Receipt/assessment events are append-only. A new paper version, changed claims,
new snapshot or revocation creates a new comparison; it never rewrites a settled
historical review. Query recovery may append results to the existing subject,
but a settled publication decision is not silently reapplied.

Private claims, complete queries, generated embeddings, cached results and
provider error details share the paper's author/staff visibility. A public
publication does not itself authorize public audit summaries, contributor access,
challenge participation or upstream feedback. Each needs its own applicable
consent. Revocation closes new access and export, while immutable historical
receipts retain their truthful provenance.

The submission and its own unconfirmed or derived claims cannot enter that
round's external authority baseline. Manuscript-local hypotheses may be used
as explicitly local premises; they never become previously established public
knowledge merely by being indexed. Reused relations must retain source consent,
conditions and lineage.

An incomplete required query cannot support sufficient novelty evidence. Other
review work may proceed, and the author may receive a letter stating the actual
limit and retry path. Existing correctness blockers and the supported
open-problem-settlement route continue to apply independently. Private
`review_only` work never creates publication records. Historical contracts are
not retroactively supplied with receipts they never had.

Author JSON, Markdown and UI must agree on the specific query status, relation,
qualification, assumptions, unresolved obligations and source links. Public
views show only independently authorized summaries, without private review
payloads or provider metadata. Challenge duration without actual participating
reviewers grants no evidence upgrade. Upstream feedback starts as an attributable,
licensed proposal package; only a verified upstream acceptance/readback event
can mark it accepted or frozen upstream.
