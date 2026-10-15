# Exact finite computation validation — 2026-10-16

The first replay protocol consumes bounded JSON vectors and a witness. It
reconstructs three rational rank-one orthogonal projections, checks the actual
matrices' symmetry/idempotence, and evaluates the complete ordered matrix
expression documented in `COMPUTATION_REPLAY.md`. No submitted code, upstream
workspace, provider or Lean execution is involved in the arithmetic checker.

The known projection example reproduces **−8/243**. Independent Python exact
fractions using rank-one dot-product contractions agree with Rust on **61**
deterministic inputs across dimensions 1–6. Negative cases include a zero
witness, identical projections, malformed/nonmatching dimensions, zero input
vectors, oversized entries/input, unknown protocol/fields and representational
overflow. A failed calculation contains no numeric success result.

The worker saves the result before Oracle admission under source/attachment,
round, claims-revision and live-lease fences. Integration tests exercise an
uncertain Oracle admission with exact prompt replay, a replaced lease, a changed
claims revision, forged input hashes, self-submitted receipts and legacy rounds.
The actual result reaches audit, advice and letter unchanged. The fixture's
unrelated manuscript remains judged on its own audit: this calculation alone
does not refute it or alter policy. Author Markdown/JSON/UI expose the same
receipt, while unrelated public readers receive no private referee access.

Validation at the final local code:

- **343 API tests** passed with real task-owned MongoDB and TeX Live.
- Strict workspace Clippy, formatting and cargo-deny passed.
- **322 Web tests** passed, together with format/lint/types, production and
  showcase builds, and dependency audit (zero vulnerabilities).
- **61 independent exact arithmetic cross-checks** passed.
- Logs: `/private/tmp/mathesis-computation-final-{tests,clippy,fmt,deny,arithmetic}.log`
  and `/private/tmp/mathesis-computation-web-full.log`.
- Two ordinary real Lean tests remain explicitly ignored locally and are
  delegated to the existing Linux CI/image gate. They validate the existing
  solving verifier, not this arithmetic claim.

The new optional manifest field and private report fields are described in
OpenAPI. Existing SDK/CLI/MCP review exports retain these fields without a new
endpoint. There is no client API for supplying a successful replay receipt.

This satisfies the bounded finite-calculation engineering and negative-case
work in P2-B. A real Oracle/CMA audit of a manuscript, human-adjudicated labels,
research-obligation solving, recognized upstream coverage certificates,
external contributions and upstream acceptance/readback remain separately
tracked. No production deployment or formal coverage certification is claimed.
