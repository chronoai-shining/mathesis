# Version-bound review follow-up

An author response requests another ordinary referee round. It does not resolve an
original finding. A completed re-review now records a separate assessment of each
response admitted into that round:

- `addressed`: the reviewer considers the original concern answered on this source.
- `unresolved`: the concern remains, with an explanation of the missing repair.
- `not_checkable`: the supplied evidence or reviewer output cannot support a conclusion.

These are model review judgments, not independent execution or Lean receipts.
The original audit remains immutable. The author can read the conclusion alongside
the original finding and response, and download referenced attachments from the
exact assessed version. Conclusions from older source/statement revisions remain
visibly historical. Both JSON and Markdown dossiers include the assessments.

## Contract and validation

`RefereeAudit.response_assessments` is additive and omitted when empty. Each entry
contains `response_id`, `status`, `explanation` and `evidence: [{path, sha256}]`.
Layer 2 requires a unique assessment for every current-version/current-claims
response admitted by the round. It rejects unknown response IDs, duplicate entries,
empty or oversized explanations and attachment references that do not match the
current immutable source inventory. A model supplies paths only; the binary binds
the actual stored hashes. An unknown attachment path cannot establish `addressed`.
Missing, duplicate or malformed model conclusions become explicit `not_checkable`
entries. Extraneous model response IDs are never projected to authors.

New rounds persist `review_contract: 2`. Earlier rounds without a contract
deserialize as contract 0. Contracts 0 and 1 retain their original prompt bytes,
including when a worker resumes an admitted task after upgrading. No historical
conclusion is fabricated. Contract 1 adds follow-up instructions only to audit
tasks with actual author responses. Contract 2 additionally transports declared
attachment text and its exact stored inventory to audit, advice and letter steps
(512 KiB per UTF-8 file, 2 MiB total). Binary or larger attachments remain explicit
inventory-only entries. The inventory is recomputed and compared with the stored
source version before it is sent. A local source-directory path does not transfer
its files to remote CMA; the bounded evidence is carried in the actual prompt.
Advice and letter generation receive the sanitized audit through their existing
interfaces; neither publication policy nor proof verification changes.

## Delivery and asynchronous cleanup

An author projection hides advice and letter drafts. Their absence means the
letter has not been delivered, not that the review has finished without one.
The progress card and review panel use the same wording for this case.

CMA acknowledges agent stopping asynchronously. Mathesis observes the agent's
actual `stopped` state before issuing a durable DELETE, avoiding an early
`agent_not_stopped` response cached under that delete key. The historical private
`stopped` progress flag records acknowledgment of the stop request; `deleted`
records observed removal (404 or a deleted state), not an asynchronous DELETE
acknowledgment. Deletion retries retain the original idempotency key. A delivered review keeps its existing queue job while
cleanup remains pending, deferring 30 seconds without rerunning the review or
spending retry attempts. The saved answer and original payer remain unchanged.
Missing agents count as already removed. Cleanup failures retain durable progress
and never substitute another payer's credentials.

On normal worker shutdown, Mathesis cancels the local polling operation and its
lease heartbeat before releasing that job back to the queue. The next worker can
resume immediately with the same admitted provider handles, without spending a
retry. If releasing the lease fails, ordinary lease expiry remains the recovery
path. A process crash still relies on that expiry; shutdown does not cancel the
remote review or create a replacement task.

## Operating the real review loop

API `/readyz` currently checks database readiness only. It is not evidence that
Oracle or CMA execution works. Before a real evaluation, check the Oracle pool,
CMA workspace/profile and a task-scoped execution; follow its terminal result.
An agent creation HTTP 201 alone does not establish execution readiness.

The existing `WISHPOOL_CMA_BASE_URL` can use CMA's published direct HTTPS API with
the same NyxID user access token when direct downstream authentication is supported.
Do not automatically switch endpoints or payer after an uncertain write. Preserve
the original body, idempotency key and task handles. In the 14 October evaluation,
a JSON-media-type refusal was replayed under its original key; a separate direct
HTTP diagnostic request with its own key was admitted. This is not a reason to
rotate keys for uncertain work or to alter NyxID/CMA upstream code.
