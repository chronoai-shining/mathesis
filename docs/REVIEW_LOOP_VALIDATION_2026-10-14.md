# Review-loop validation — 14 October 2026

This evaluation uses a private local Mathesis instance, MongoDB, a real research
manuscript and its actual attachments, the live Oracle broker, and the live CMA
API. It is not a production deployment or a publication decision. Source research
repositories are read-only; copies and diagnostic artifacts live outside Mathesis.

## Implementation and automated checks

- `5f942ad1918e9be5c5abd59f6a9ee89f2664b033` binds new re-review conclusions to
  admitted author responses and the exact source evidence inventory. It adds
  author-facing conclusions and exports, preserves historical review prompts,
  and observes asynchronous CMA stop/delete completion.
- `8648abf8935673e0e9ca7c4593e7be875238948b` releases jobs on normal worker shutdown
  after dropping local work and its heartbeat. Provider identities are preserved.
- `4ecfc38` fixes a real evidence-delivery omission: the local source directory
  was not transferred to CMA's audit, advice or letter prompts. New review
  contract 2 sends the actual bounded attachment contents and hashes. Contracts
  0 and 1 keep their original admitted inputs. The regression covers all three
  prompt types, binary coverage, stored-inventory mismatch, unchanged legacy
  prompts and the larger advisor text budget.
- `33068d79064c405e7b00c9362da18850eca16aa0` makes the private review headline and
  badge follow the current round's letter delivery, and renders mathematics in
  finding titles and feedback subjects.
- `ce5fb5b` includes the stable author-response ID and claims revision in the
  readable Markdown dossier. Both real export formats were checked against the
  saved assessment and the actual attachment hash.
- Backend: 271 tests passed with MongoDB and TeX integration enabled; fmt and
  all-target/all-feature clippy passed. The earlier complete gate also passed
  cargo-deny. Frontend: 305 tests, typecheck, lint, formatting, build, and the
  high-severity dependency gate passed. Existing low-severity dependency notices
  and the existing bundle-chunk warning are not treated as new failures.
- A fake-provider regression completes two private review rounds, two delivered
  letters, an author response, a version-bound assessment and safe dossier
  exports. It checks unchanged original findings and idempotent replay. These
  assertions establish application behavior, not live-provider availability.
- Browser checks at 320 px and 1440 px verified response conclusions, versioned
  evidence links and absence of horizontal overflow or JavaScript errors. Review
  results in that UI test were explicitly synthetic; the private source/API
  authentication was real. The synthetic view is not live mathematical evidence.
- CI and CodeQL passed for `5f942ad`. Links:
  [CI](https://github.com/the-omega-institute/mathesis/actions/runs/38556692000),
  [CodeQL](https://github.com/the-omega-institute/mathesis/actions/runs/38556691851).
- CI and CodeQL also passed for `8648abf`:
  [CI](https://github.com/the-omega-institute/mathesis/actions/runs/38557660112),
  [CodeQL](https://github.com/the-omega-institute/mathesis/actions/runs/38557653649).
- The evidence transport fix `4ecfc38` also passed
  [CI](https://github.com/the-omega-institute/mathesis/actions/runs/38558662452)
  and [CodeQL](https://github.com/the-omega-institute/mathesis/actions/runs/38558662445).
- The UI follow-up `33068d7` passed
  [CI](https://github.com/the-omega-institute/mathesis/actions/runs/38559927349)
  and [CodeQL](https://github.com/the-omega-institute/mathesis/actions/runs/38559926805).
- The readable-export follow-up `ce5fb5b` passed
  [CI](https://github.com/the-omega-institute/mathesis/actions/runs/38563654466)
  and [CodeQL](https://github.com/the-omega-institute/mathesis/actions/runs/38563653872).

## Real submission and evidence boundary

Submission `01a13a18-4a7b-7210-ac0e-b180206ea23b` is private and uses
`purpose: review_only`. Version 1 failed compilation because the local TeX
executable was not configured. Version 2 resubmitted byte-identical source after
correcting that runtime setting and confirmed three claims. The failed first
round remains in its history.

The archive carries three actual attachments. The forcing-witness JSON has SHA-256
`0c000ef766154da8e5b61a792c209c476afd00d38cf34ff3883139b74ce25ef1`.
A separate bounded local checker replayed all 22 supplied forcing traces and
rejected a deliberately corrupted trace. It did not execute the uploaded script.
This supports the supplied finite upper bounds only; it establishes no matching
lower bounds, missing refutations, universal theorem or Lean verification.
This local diagnostic is not an automated Mathesis execution service.

Oracle task `72ef3477-20df-4920-a54e-52f87d6ab993` completed at
2026-10-14 11:26:20 UTC with provider-reported model `gpt_6 · pro` and a
major-revision recommendation. It identified missing computational evidence
needed for the manuscript's lower bounds, while accepting the supplied traces
as upper-bound evidence. Its model-reported checks are not independent verifier
receipts. Mathesis saved the real report and admitted the CMA audit automatically.

## Restart and resource lifecycle

The first audit retained agent `agt_161e865cc2f8402ca199b39a6b961e0a` and response
`resp_f6981d144c7542b19f4c770a0284e584` across worker restarts. A prior executable
left its lease behind on graceful exit. After confirming that all affected local
Mathesis processes had exited, the evaluator expired that exact old lease in this
test database once. This was explicit local recovery, not automatic failover.

With `8648abf`, a further SIGTERM at 11:35:20 UTC returned the running job to the
queue within approximately 4 ms, and the replacement resumed the same agent and
response. Queue retry accounting was unchanged by this graceful shutdown. A
paused-clock regression separately confirms immediate recovery, unchanged Oracle
results and no late write or detached heartbeat.

The diagnostic agent `agt_d0bdd10b70654effb6162fb6d7524dde` was observed as deleted
after asynchronous stop and deletion. A 202 response alone was not counted as
cleanup. Future review steps use a dedicated empty Mathesis workspace,
`wks_e6e491aebd0d4853820e37c32adbafd0`, created through CMA's existing published
interface. The already-admitted audit retains its original workspace. No shared
sandbox/profile, upstream code or unrelated agent was changed.

## Live delivery and re-review

The real CMA audit completed and was filed by Mathesis with a major-revision
assessment. It also reported that the externally supplied forcing JSON was
unavailable. Inspection confirmed a downstream transport omission: Oracle had
received that attachment, while CMA's advisor prompt contained only the manuscript
text. The immutable audit is retained as delivered; its mistaken availability
finding is the subject of the author's submitted re-review response. Fixing the
transport does not repair the genuinely missing lower-bound certificates.

The first real letter was delivered automatically at 11:48:43 UTC. The successful
round began at 11:08:07 UTC, so elapsed time was approximately 40 minutes 36 seconds,
including provider work, queue waits and intentional local restarts. This is one
development evaluation, not a production latency benchmark or service guarantee.

| Round 2 milestone | Observed completion (UTC) |
| --- | --- |
| Oracle report | 11:26:20 |
| CMA source audit | 11:37:12 |
| CMA improvement advice | 11:46:42 |
| Delivered feedback letter | 11:48:43 |

Author response `d296810f59642a50be41422239d0e6d01f04304a319e5cafbcd190c4dfe90f67`
asks the reviewer to inspect the already-uploaded JSON and correct its availability
finding. It explicitly concedes that the missing lower-bound material has not
been supplied. Round 3 admitted that response under contract 2, on unchanged source
version 2, with Oracle task `21c15391-0a50-41d5-a5ed-bcade2f3bfb0`.
That Oracle review completed at 12:08:20 UTC. It explicitly corrected the attachment
availability finding, reported replaying all 22 traces and 661 forces, and retained
major revision because the lower-bound evidence was still missing. Its claimed
computations remain model findings. Mathesis automatically admitted CMA audit
agent `agt_146051d820674a66b3c3012a21e23db7`, response
`resp_9b0f1d9734434f3ebb229250f608d64f`, in the dedicated Mathesis workspace.
The CMA audit completed at 12:21:21 UTC and recorded `addressed` for that exact
author-response ID. Its evidence reference resolves to
`evidence/data/forcing-witnesses.json` and the actual stored SHA-256 above. The
explanation explicitly limits this to resolving attachment unavailability, with
no lower-bound proof established. The audit still recommends major revision.
An exact JSON comparison confirms that round 2's original audit was not modified.

The new audit additionally reports its own finite enumeration and dynamic-program
cross-checks. These are model-reported computations in a temporary provider
workspace, not authenticated execution certificates or independently verified
Lean results. The platform does not upgrade them into proof receipts.

Real browser testing during this pending re-review found that the headline and
status badge could still say the review was ready because the prior round was
complete. They now follow the current round's delivery state. A previous letter
cannot mark a later round delivered. Findings and letter subjects also use the
existing mathematical-text renderer. Authenticated, unintercepted checks at
320 px and 1440 px verified pending response status without an invented addressed
conclusion, rendered formulas, no page overflow and no browser errors.

An exact retry of the author's response during the live re-review returned the
same response ID, one trigger and the same round. All three first-round CMA agents
were subsequently observed deleted through the normal Mathesis cleanup path.

The second real letter was delivered at 12:34:38 UTC, approximately 45 minutes
19 seconds after round 3 began. Its recommendation remains major revision; it
acknowledges the supplied forcing traces and clearly requests the still-missing
lower-bound materials. Both letters are private in-app feedback. No acceptance,
public record, winning contribution or independent Lean receipt was created.

| Round 3 milestone | Observed completion (UTC) |
| --- | --- |
| Oracle re-review | 12:08:20 |
| CMA source audit and response assessment | 12:21:21 |
| CMA improvement advice | 12:32:34 |
| Delivered feedback letter | 12:34:38 |

Final live assertions passed:

- Three historical rounds: the retained initial compilation failure and two
  successful real reviews; exactly two delivered letters and one author response.
- The original audit is byte-for-byte equivalent after JSON normalization.
- The new audit records one `addressed` conclusion for the admitted response,
  with a concrete explanation and an actual attachment path/hash.
- Downloading that attachment reproduces its hash and 29,653-byte length.
- Author JSON and Markdown exports contain the same response ID, explanation
  and evidence hash, use `no-store`, and omit provider handles and creation intents.
- Another signed-in identity receives 404 for the referee view, both report
  exports, and the attachment download.
- Retrying the exact author response after delivery creates no duplicate response
  or round. Purpose remains `review_only`, status is `review_complete`, and there
  is no publication decision.
- A final browser run used real authenticated API responses, without interception,
  and confirmed both letters, the displayed response conclusion and evidence links
  at 320 px and 1440 px without overflow or browser errors.

## Cleanup observation and remaining operating limits

At the original observation, five of six agents were deleted and the second
advice agent was still awaiting asynchronous cleanup. A follow-up observation on
15 October (Asia/Singapore) confirmed all six persisted agent steps have
`deleted: true`, no recorded failure, and the dedicated database has no remaining
review jobs. The existing worker completed cleanup without a new review or a
manual database mutation. The previous pending agent was
`agt_6d20d26dd7594f2aa58887a94ed9e4c1`.

The complete review-and-response delivery loop is demonstrated in this local
evaluation. Production rollout, broad latency/load characterization, an automated
computational-certificate executor and arbitrary Lean-project verification are
not established by it. Model-reported research computations remain review findings.
The prepared-workspace independent Lean verifier remains a separate trust boundary.
