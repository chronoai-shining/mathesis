# Research obligations — implementation contract

This is the P2-A implementation contract. Implementation and live acceptance
remain tracked in `CAPABILITY_DELIVERY.md`; this document is not a completion
receipt.

An unresolved hypothesis or bridge in a saved source audit can become a precise
private research obligation. Preserve the original submission/version/claims
revision/round, complete joint premise set, parameter mapping, actual query
receipts, and the exact unresolved text. The author supplies or confirms the
complete mathematical goal, including definitions, quantifiers and assumptions.
An authorized task does not rewrite the source audit or declare that a missing
hypothesis is true.

The initial path stays private to the submitting author, linked co-authors and
staff. Acceptance/public visibility is not permission to expose review evidence.
Contributor access and public summaries are independent P3 permissions. A private
research goal need not become a public accepted conjecture just to use the Lean
checker. Its receipt receives no conjecture/formalization leaderboard credit.

Use the existing target → attempt → independent verifier protocol and carriers:

1. An author adopts one exact current audited obligation, confirms its complete
   goal and a bounded attempt budget/deadline, and explicitly authorizes remote
   target preparation under the existing processing consent.
2. The existing formalizer translates the precise goal to `Target.lean`, the
   local prepared checker elaborates the returned bytes, and a separate managed
   CMA inspection checks faithfulness. A missing or non-faithful inspection
   cannot unlock confirmation. One regeneration is allowed within the recorded
   preparation attempt; unknown external outcomes resume their original handles.
3. The submitting author confirms the exact target digest in a cookie session.
   A changed source, goal, target or consent invalidates new work on the old pin.
4. Attempts use the existing `Attempt`, module/source bounds,
   `VerificationRequest`, credential-free verifier and strict receipt checks.
   Solutions/modules are GridFS objects. No user/editor/model endpoint can file
   a successful receipt. Each distinct admitted attempt consumes the task budget;
   exact retries resume it, and expiration/revocation closes new admissions.
5. A verifier result is appended to the originating private review history under
   the live lease and revision fences. It binds the goal and exact target; it
   does not automatically discharge other hypotheses or establish original-claim
   correctness, source coverage, novelty, publication or credit.
6. A fresh ordinary re-review may inspect the result as internal mathematical
   evidence. It must explicitly audit how the checked target relates to the
   remaining bridge. Never insert a manuscript's own derived result into its
   external Trureturing baseline or fabricate an upstream certificate.

Persistence uses the existing referee header plus checksummed GridFS history.
Keep task admissions, target attempts, author decisions, solving attempts and
verification outcomes append-only. Jobs use the existing review and verifier
lanes; new job variants are local application work, not new shared infrastructure.
The binary composes Layer 2 authorization/state with Layer 3 carriers. Layer 2
contains no filesystem/network calls or dependencies on other workspace crates.

Validation must cover private access, exact adoption/retry, missing hypotheses,
target faithfulness and cookie confirmation, exhausted/expired/revoked work,
uncertain remote steps, stale source/revision/lease, forged receipt identity,
duplicate verification and private result export. Ordinary real Lean checks stay
in the existing Linux verifier gate. A fabricated fixture audit is an engineering
test, not a live mathematical review or external participant's authorization.

## API and clients

All routes are under `/api/v1/submissions/{id}/research`. GET returns current
source-audited candidates and the append-only private histories, with projected
state, current target and attempts. POST adopts a displayed candidate. The
submitting author alone can adopt, stop, decide a target, submit an attempt or
request a re-review. Linked co-authors and staff can read. Other callers get 404.

An adoption body is:

```json
{
  "round": 1,
  "claim": "C1",
  "gap": {"kind": "hypothesis", "index": 0},
  "candidate_digest": "sha256:<the displayed candidate digest>",
  "goal": {
    "definitions": ["Define every symbol needed by the goal."],
    "hypotheses": ["Only the assumptions actually available in the manuscript."],
    "quantifiers": "State all variables, domains and quantifiers.",
    "conclusion": "The exact proposition to prove or refute.",
    "correspondence": "Explain how this proposition addresses the selected gap."
  },
  "budget": {"max_attempts": 2, "expires_at": "<future RFC3339 time within 30 days>"},
  "consent": "research-obligation-2026-10-16"
}
```

`POST /{key}/target/response` accepts `digest`, `confirm` and optional `comment`;
confirmation requires both a faithful independent inspection and a cookie
session. The target source and toolchain are returned by GET. Rejection requires
a correction. Target preparation allows at most three recorded generations,
with one inspection-directed regeneration per generation. Unknown remote
outcomes reuse their existing durable request identity; retries do not create a
fresh agent conversation merely because the worker restarted.

`POST /{key}/attempts` accepts `target_digest`, `solution`, optional `modules`
(dotted module name to complete source) and `note`. It uses the existing 1 MiB
solution, 512-module / 8 MiB source bounds and verifier protocol. Supply
`wishpool_solution : wishpool_target_prop` or the existing disproof form
in a Lean module importing `Target`. A repeated source/target pair resumes one
attempt. There is no route to submit a successful verification receipt.

`GET /{key}/attempts/{attempt}` returns exact saved solution/modules and the
actual verifier receipt. `POST /{key}/attempts/{attempt}/recheck` requests a fresh
ordinary source audit of a checked result on the current source. A frozen
recheck trigger selects the immutable original goal, target, source binding and
verified attempt; previously admitted prompts do not acquire this evidence.
`POST /{key}/stop` requires a bounded `reason`, closes live CMA authorization and
retains history. Expired, withdrawn, revised or subsequently settled inputs also
close new work; late verifier results cannot be filed under a lost lease.

The author workspace offers these same actions. The private JSON review dossier
(schema 3) and Markdown export retain goals, source binding and checked result
identities; proof sources remain available from the private attempt route.

CLI commands, also available as MCP tools with the same names:

```text
mathesis-contribute research SUBMISSION
mathesis-contribute adopt-research SUBMISSION obligation.json
mathesis-contribute research-attempt SUBMISSION OBLIGATION attempt.json
mathesis-contribute research-attempt-status SUBMISSION OBLIGATION ATTEMPT
mathesis-contribute recheck-research SUBMISSION OBLIGATION ATTEMPT
mathesis-contribute stop-research SUBMISSION OBLIGATION "reason"
```

Exact-target confirmation stays in the signed-in browser, consistent with the
existing public-target protocol. Neither CLI nor MCP bypasses this author step.
The operator remains the fallback payer under the existing CMA selection rules;
research generation and faithfulness inspection are eligible for the submitting
author's connected CMA account. Research work adds no public supporter credit.

## Engineering validation and live acceptance

The automated fixtures test implementation behavior, including fake independent
verifier and fake formalizer/inspector boundaries. They do not certify that the
fixture mathematics is true or constitute real external author participation.
The existing Linux verifier image gate continues to run actual Lean proof tests.
A complete live research-obligation acceptance run additionally needs a ready
managed CMA workspace and an actual author's exact goal/target authorization.
No such live receipt is fabricated by this implementation.
