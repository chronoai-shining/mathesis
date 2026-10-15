# Engineering validation — 15 October 2026

Local changes were prepared against `develop` at
`80859bd7ed602ab99b7037728cd79d2a01db76fa`. This report describes local
verification; it does not claim a production deployment or a new live model run.

## Fixes and regression evidence

| Change | Evidence |
|---|---|
| Block publication when another main result has an explicit error, gap or unchecked audit | Policy tests and a real HTTP submission with two main results: the positive result no longer overrides the false `0 = 1` result; preview returns `not_accepted` and `incorrect_result` |
| Highlight confirmed errors on published work | Domain checks cover pending versus confirmed reports, private visibility and different source versions; browser verifies the notice in the production web bundle |
| Preserve large referee histories outside the BSON document limit | Real MongoDB test saves and reads more than 18 MB of history, checks revision fencing, preserves source blobs and rejects stale replacement |
| Offline orphan recovery | MongoDB tests retain referenced/recent files, refuse live leases, collect old orphan files and aged interrupted-upload chunks |
| Isolate long reviews from compilation and verification | Lane claims tested against MongoDB; a running memory worker dispatches another submission's compile job while its sole review lane remains in a delayed audit |
| Require CMA profile and check execution prerequisites | Configuration and read-only transport tests; missing auditor starts no Oracle task; existing account, replay, cleanup and token-redaction tests pass |
| Honest readiness | Local `--check` reports working TeX and disabled model services; missing TeX returns unavailable/nonzero; API-only readiness remains independent of remote execution |
| Real verifier gate | Two explicitly selected real Lean tests pass, including proof, disproof, wrong type, forbidden axiom and supporting modules; ordinary runs explicitly report them ignored |
| KaTeX security update | Root and transitive dependencies resolve to 0.18.10; all web tests pass; production browser check renders formulas without errors; the manual chunk rule avoids a new circular dependency |
| Static Pages showcase | Browser checks at 1440, 768, 390 and 320 px: no horizontal overflow, failed asset requests or script errors; section navigation works; the default build makes no application API/auth requests |
| Optional custom domain and application link | A separate root-path build loads in the browser and shows the configured HTTPS application link; the default project build has no CNAME or live-service button |

## Completed checks

- API: format and strict Clippy; 278 ordinary workspace tests passed, with real
  MongoDB 8.0.17 and TeX enabled. Two real Lean tests were separately selected
  and passed. The additional publication-privacy regression also passed after
  being extended to cover different versions.
- SDK: format, strict Clippy and all three tests passed.
- Web: locked install, format, lint, typecheck, all 310 tests, application build
  and showcase build passed. `npm audit --audit-level=high` reports zero known
  vulnerabilities after the KaTeX upgrade.
- `cargo deny check`: advisories, dependency rules, licenses and sources passed.
- Gitleaks: no leaks in the current tracked and untracked source snapshot.
- Kubeconform: all 11 resources across nine manifest files valid, none skipped.
- CI structure scripts, layer boundaries, configuration-key declarations,
  workflow YAML parsing and `git diff --check` passed.
- Real HTTP smoke: development authentication, source upload, extraction,
  outsider 404, TeX PDF compilation/download, statement confirmation and the
  corrected decision preview passed.

Local toolchain: macOS arm64, Rust 1.95, Node 22.22.2. Real Lean tests used an
isolated copy of already prepared Lean 4.35.0-rc3 / Mathlib
`5e0c4e5239cb0a2d86d68a884bf52cfd963fce22` artifacts. The source project supplying
those artifacts was not modified. Temporary testing services and the copied
Lean workspace are cleaned up; the project showcase preview may be left open
for review.

## Not yet verified or deployed

- The NyxID session was renewed and read-only CMA access confirmed. The available
  inspected sandboxes were retiring/retired; no usable company-managed workspace
  was provisioned. No paid Oracle review or CMA agent was started for this repair.
  Live end-to-end acceptance still requires usable workspace/profile/model resources.
- Docker is not installed in this environment. The new prepared-base and verifier
  image build/test stages are prepared as source changes for CI, but their
  Linux container execution has not been run locally or on GitHub. The reference
  Mathlib commit's Lean 4.33.0 toolchain pin was checked against upstream.
- These changes are submitted together for review; no Pages settings change,
  custom-domain registration or public deployment is part of the PR. The expected Pages URL is
  `https://the-omega-institute.github.io/mathesis/` after deployment.
- The complete editorial correction/retraction lifecycle, mathematical quality
  evaluation, automatic CMA provisioning and missing upstream usage receipts
  remain the product/infrastructure boundaries described in
  [OPERATIONS.md](OPERATIONS.md).

For rollout and storage-format rollback requirements, read
[OPERATIONS.md](OPERATIONS.md). For publishing and domain setup, read
[GITHUB_PAGES.md](GITHUB_PAGES.md).

## Methodology upgrade validation

The same change set additionally introduces review contract 3 and
`trureturing-content-2026-10-15`; see [REVIEW_METHODOLOGY.md](REVIEW_METHODOLOGY.md).
Regression cases cover a direct-conclusion witness, missing live-use evidence,
consumed bind-only helpers, unavailable literature, finite positive enumeration,
checkers/numerical reductions with undischarged premises, genuine counterexample
utility, exact named-problem matching, malformed citations, independent optional
information-gain analysis, conjecture evidence and historical contract replay.

The upgraded code passes 290 ordinary API workspace tests with real MongoDB and
TeX, strict API Clippy, and 312 web tests with format/lint/typecheck and both
application/showcase builds. The new tests use synthetic model responses; they
do not establish the quality of a live mathematical review. The two real Lean
tests remain explicitly ignored in ordinary runs; the earlier separate Lean
validation above used a different prepared toolchain from the pinned CI image.

An initial MongoDB test run hit the host's free-space floor. Removing only
regenerable project build artifacts restored space, and the database tests
passed on rerun. No runtime database was reset or replaced.

## P0 integration verification — 16 October 2026

The methodology branch was merged with develop at
`1e9b7f7fb3b981d3ca5079a0600bbe925eaa3455` in an isolated worktree. Static review
found that offline collection checked whether a review snapshot existed but
did not read it before collecting other files. The collector now shares the
ordinary snapshot reader, checking its format, checksum, submission identity
and revision before any deletion. References in the decoded history are retained.

The new MongoDB regression
`offline_collection_refuses_unreadable_review_snapshots_before_deleting_files`
failed on the original implementation at the checksum mutation. With the fix,
checksum, missing-blob, revision, non-string-format and unknown-format mutations
all prevent audit/collection and preserve the recovery file. Restoring the
header allows ordinary collection and leaves the history readable.

Local API verification passed 291 tests with an isolated MongoDB on port 27031
and real TeX, plus formatting and strict all-target/all-feature Clippy. The two
real Lean tests are explicitly ignored locally. Web formatting, lint, types,
313 tests and both builds passed. Cargo deny passed; npm's locked installation
reported zero vulnerabilities. The fix's diff passed gitleaks and whitespace
checks. No provider, production database or upstream repository was modified.

The earlier GitHub run
[`38744507574`](https://github.com/the-omega-institute/mathesis/actions/runs/38744507574)
at `e3c4d0adedd3c641b2809694156b1baa35c43c7b` successfully built the prepared
Linux verifier and ran real Lean proofs. This supersedes the earlier local-only
image limitation for that revision; it is not a claim about checks on the new
integration commit, live model quality, or a production deployment.
