# Binding contributions: engineering validation

P3-A is implemented as a Mathesis downstream service. It consumes saved source
queries and creates no upstream changes, publications or coverage certificates.
The public path requires an accepted public publication, enabled contributor
access and a separate exact summary authorization. Author/co-author discovery
claims are refused. Five admitted contribution/challenge requests per person in
24 hours are shared across discussions and reserved atomically in MongoDB.

Local validation on 2026-10-16:

- 369 API tests passed with the task-owned MongoDB at port 27031 and TeX Live.
  The two existing actual-Lean tests remain ignored locally; the existing Linux
  verifier-image workflow is the actual Lean gate.
- Six new core tests cover explicit separate consent, author exclusion, source
  and visibility fences, idempotent and concurrent admission, daily limits,
  complete inspected premise identities, lost leases, pending challenges,
  reversible effective credit, privacy and first-supported priority.
- A worker test transports exact checksummed saved source responses into the
  independent inspector. Its truncated-source variant receives unclear without
  calling the inspector. It also checks `RunStore::active` after revocation.
- Two real Mongo tests exercise concurrent global admission, replay and expiry,
  and directory metadata on a checksummed GridFS referee history. A missing
  snapshot is an error rather than a fabricated empty history.
- An HTTP integration test exercises public/private projections, absent and
  outsider authentication, rejected client verdict fields, author exclusion,
  public profile output and revocation of public reads and new submissions.
- 328 Web tests passed, including three new form tests for unchecked exact
  summary consent (reset by edits), structured licensed binding submission and
  explicit contribution-bound challenge/appeal submission. Typecheck, strict
  lint, format, production build, showcase build and dependency audit passed.
- Three SDK tests, strict Clippy and format passed. The existing MCP inventory
  test includes the eight new matching REST service tools.
- API strict workspace Clippy/fmt and cargo-deny passed. A focused binding test
  pass followed the final canonical-target/history-budget tightening.

The independent inspector in tests is a synthetic fixture. These results do not
establish actual external participation, mathematical CMA acceptance or a live
challenge outcome. Those acceptance cases remain unachieved until real actors
and actual managed inspection produce their own receipts. No production
capability deployment is claimed here.

Public JSON deliberately excludes private inspection reasoning and raw source
query receipts. The author's JSON/Markdown dossier and private workspace retain
the complete append-only history. Effective binding credit is calculated from
independent audit events, including challenges, and remains separate from
Formalized/Solved proof receipts. Neither a quiet discussion nor a waiting
period grants a novelty status.
