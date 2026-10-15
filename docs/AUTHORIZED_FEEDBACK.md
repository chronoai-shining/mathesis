# Authorized feedback proposals and publication readback

P3-B belongs entirely to Mathesis. It prepares a licensed proposal package for
existing upstream admission routes and consumes actual published evidence. It
never creates or edits an upstream issue, pull request, branch, source, check,
release or artifact. A package is not an upstream acceptance or freeze receipt.

## Exact export and separate rights

A proposal is pinned to an accepted public Mathesis publication, confirmed claim
set and source digest. Its producer identity, mathematical payload, original
attribution, evidence scope, requested route and license are immutable. The
submitter previews the complete payload before admission. Public discussion
consent is not upstream-export consent.

The supported intake channels are:

| Channel | Exact Mathesis evidence | Required rights holders |
| --- | --- | --- |
| Open problem preregistration | Public independently admitted open problem; full statement and original source/record | Submitting author |
| Verified solution | Current confirmed Target plus saved independently verified solution/modules and receipt | Submitting author and solution owner |
| Library binding | Current independently supported licensed joint binding and approved public summary | Submitting author and binding contributor |
| Witness formalization | A currently public audited content witness on an accepted result | Submitting author |
| DTR registration proposal | Verified result plus explicitly supplied four-slot proposal, labelled as proposed registration | Submitting author and solution owner |
| Erratum | Confirmed public error report, exact subject and supplied upstream identity | Submitting author and reporter owner |

Original authors, formalizers, owner/agent attribution and original licenses
remain in the package. Each required owner must explicitly authorize the exact
payload digest and assert the applicable rights. Trureturing's inspected LICENSE
is Apache-2.0 (at source `e7dcc89974fd44cabd0f10781eccc135eddf8d1c`, bytes SHA-256
`45404ee75576b053a86e89e545fd6987a607107cf4d846fa78dcb6a233d53447`). A CC-BY-4.0
public binding does not acquire an Apache-2.0 export grant automatically.

Preparation never fabricates consent. Missing consent blocks export. Approval,
export and revocation append events. Source changes, lost public visibility,
withdrawal, a void target or revoked binding/error evidence stop new export.
Previously licensed/exported copies remain governed by their license. Large
payloads use GridFS, with the same checksum and revision protections as other
Mathesis research evidence. Public/private readback uses explicit projections.

The package identifies its Mathesis producer operation and exact verification
scope at the top of its Markdown proposal. It preserves exact natural-language
statements, source/claim identities, proof toolchain and verifier receipts where
available. It does not invent D5 modules, registration ids, first-tier source
eligibility or a successful upstream admission gate.

## Observations are not user assertions

An author may attach an upstream PR reference and exact declaration identities
as an observation request. That request remains an assertion until Mathesis
actually reads the source. There is no client endpoint for success receipts.

Readback uses two independent existing interfaces: a read-only GitHub PR/commit
observation and the immutable operator-admitted publication/index already stored
by Mathesis. Retain fetched identities, hashes, timestamps and failures. Verify
that a claimed merged PR belongs to Trureturing, targets its existing dev branch,
and that its merge commit is included in the consumed source commit. A URL or
open PR does not establish acceptance.

For each requested declaration, check its full identity in that admitted index
and preserve its original source hash and freeze status. Do not infer declaration
proof dependencies from module imports or frozen prerequisite nodes. Searching
for an exact proposal digest in the actual module can establish an explicit
upstream provenance reference; the absence of that link remains explicit. Such
presence is not a proof of semantic equivalence or a coverage certificate.

The resulting view keeps separate facts: PR merge observation, inclusion in the
consumed source, actual declaration presence, explicit proposal provenance and
original freeze status. The interface keeps these facts separate: a digest marker is an explicit module
provenance claim, and does not prove that the referenced PR introduced the
proposal or that the declaration realizes its mathematical content. There is
no aggregate accepted/reimported badge. Mathematical correspondence not verified
by an admitted checker retains its source-level qualification. Ordinary Mathesis proof credit,
binding credit and upstream observation stay separate.

No current authored fixture counts as actual external acceptance, freeze or
credit. The live P3-B acceptance case remains unachieved until a real authorized
proposal is actually admitted upstream and appears with matching provenance in
a later independently admitted downstream snapshot.

## Interface and operations

The web submission page offers all six preparation channels and an exact package
preview. Its preparation checkbox starts unchecked. The resulting protected link
can be given to each listed rights holder; their separate Apache-2.0 rights and
export checkbox also starts unchecked. Only after all approvals can they download
the JSON package, including a proposed Markdown cover. No button or SDK operation
submits anything to Trureturing. Raw saved observations can be inspected with their
checksums; private histories retain authorization and revocation events.

All REST paths below are under `/api/v1`:

| Operation | Route | CLI/MCP |
| --- | --- | --- |
| Private history directory | GET `/submissions/{id}/feedback` | `feedbacks` |
| Exact preview | POST `/submissions/{id}/feedback/preview` | `preview-feedback` |
| Prepare immutable package | POST `/submissions/{id}/feedback` | `prepare-feedback` |
| Inspect package | GET `/submissions/{id}/feedback/{key}` | `feedback` |
| Separate rights authorization | POST `…/{key}/authorize` | `authorize-feedback` |
| Licensed download | POST `…/{key}/export` | `export-feedback` |
| Stop new serving/export/work | POST `…/{key}/revoke` | `revoke-feedback` |
| Request actual observation | POST `…/{key}/readback` | `feedback-readback` |
| Read original observation evidence | GET `…/{key}/observations/{request}` | `feedback-observation` |

`preview-feedback <submission> <route.json>` takes a `FeedbackKind` object;
`prepare-feedback` takes `{kind, preview_digest, preparation_consent}`. The consent
version is `feedback-prepare-2026-10-16`. `authorize-feedback` takes
`{digest, license, consent, rights_asserted}`: the license must be `Apache-2.0`,
consent `feedback-export-2026-10-16`, and applicable-rights assertion explicitly
true. CLI and MCP never supply consent automatically. A rights assertion records
what the holder explicitly declares; it is not independent legal adjudication.

`feedback-readback` takes `{pull_request, index_revision, declarations,
observation_key}`. Declarations are 1–20 distinct complete existing
`KnowledgeDeclarationRef` objects, and the index revision is a full `sha256:`
identity already admitted by the operator. Exact retries reuse an intent. Reusing
a key for another request is rejected. At most 10 explicit observations and 40
immutable proposals per submission are admitted. Each proposal allows 100 normal
events; revocation is still admitted after that cap. Packages are at most 12 MiB.

Readback uses the existing review lane but needs no Oracle/CMA execution or payer.
Reconciliation recovers a persisted intent after enqueue failure. The job lease,
current source, evidence and export authorization fence result recording. Ordinary
observation failures settle as a saved non-result; an explicit new key requests
a later check. Uncertain database failures remain ordinary durable job retries.
No provider outage is converted into an acceptance, absence or freeze claim.

The public GitHub API is used without credentials, with redirects disabled,
30-second request deadlines and 350,000 bytes per response. Public API quotas
therefore apply. The consumed source commit and tree come from the admitted
snapshot, and its full checked index supplies declarations and original statuses.
An exact line `Mathesis-Proposal: <payload digest>` in a module (optionally a Lean
line comment) is retained as an explicit source-level provenance reference only.
This is an optional observed reference, not a new required upstream artifact
contract. No evidence implies that module imports are proof dependencies.

Upgrade the API and worker together; old workers do not recognize
`feedback_readback`. No environment key, required CI check, default runner,
upstream publication workflow or production image tag changes in this feature.
