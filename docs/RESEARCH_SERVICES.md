# Research services architecture

Mathesis is a mathematical research archive with review as a core service.
Readers discover papers and conjectures. Authors receive an Orchestra review;
contributors can deposit existing Lean work with attribution and request
independent verification. CMA is an execution adapter, not an account users
must understand before submitting research.

## Architecture and responsibility

The implementation follows CMA's interface/domain/adapter separation and
NyxID's identity and service-connection model. The inspected references are
CMA's `CLAUDE.md`, NyxID's `docs/quickstarts/mcp-wrapping.md`,
`docs/MCP_DELEGATION_FLOW.md`, CLI proxy reference and OpenAPI parser. No upstream
CMA or NyxID changes are required by this change. Reference revisions:
[CMA 1c71725](https://github.com/ChronoAIProject/cma/blob/1c717253d82eb4fbeecbb1eae3738d0734bceaef/CLAUDE.md)
and [NyxID 84916e2](https://github.com/ChronoAIProject/NyxID/blob/84916e2d2fccbd1aacffe0614fe02d24329cbaf1/docs/MCP_DELEGATION_FLOW.md).

```mermaid
flowchart TD
  Web[Research website] --> REST[Mathesis REST and OpenAPI]
  Agent[Agent or MCP client] --> Nyx[NyxID identity and proxy]
  Nyx --> REST
  CLI[CLI or stdio MCP] --> REST
  REST --> Domain[Domain services and authorization]
  Domain --> Mongo[MongoDB aggregates and indexes]
  Domain --> Grid[GridFS sources and PDFs]
  Domain --> Queue[Durable leased work]
  Queue --> Orchestra[Orchestra review and reasoning]
  Orchestra --> Providers[Oracle and CMA adapters]
  Queue --> Verifier[Independent credential-free Lean verifier]
```

MongoDB was already Mathesis's authoritative store. This work extends it with
archive indexes and a `lean_deposits` collection; it does not introduce a
parallel database, identity provider or proof-acceptance mechanism.

| Service | Current implementation | Durable evidence |
|---|---|---|
| Paper discovery | Indexed search of Mathesis publication metadata, author/MSC/type filters, cursor pages | Immutable accepted record and publication snapshot; live visibility checked again |
| Review | Existing referee → source audit → policy decision → advice → letter pipeline; progress and audit now visible to authors before the letter; downloadable dossier | Source digest, paper version, claims revision, append-only rounds and delivered letters |
| Lean intake | Standalone source deposit with original authors, mathematical statement, source reference, toolchain and license | Immutable GridFS source/digest and attributed deposit; exact replays return the same id |
| Lean verification | Owner links a deposit to an existing confirmed public target | Ordinary durable attempt and independent verifier receipt; CAS-fenced attempt links |
| Machine access | REST, OpenAPI, expanded stdio MCP and CLI; optional NyxID CLI transport | Same authenticated domain operations as the website |

## Discovery semantics

`GET /api/v1/papers?q=prime&author=Smith&msc=11&kind=paper` searches the full
archive, not the currently displayed page. `q` is plain words; matching any
word selects a candidate. It searches title, author names, accepted abstract
and DOI. An exact `WP-…` query selects that record. Author matching is a literal,
case-insensitive substring; Mongo regex metacharacters are escaped. MSC is a
classification prefix. Results use descending record ids and `next_before`.
Keep the same filters when following a cursor.

MongoDB uses `archive_text_v1` with language `none`, so it does not stem words
or discard English stop words. Separate kind/id and publication-MSC/id indexes
support filtered browsing. The search aggregate joins the indexed submission
id to its live public/private visibility before limiting a page, with a
10-second database deadline. Layer 2 rechecks visibility on projection. Hidden
candidates do not produce extra pages or cursors. Queries are bounded and
cannot inject Mongo operators. The dev memory adapter implements plain-word
matching; it is not a production search engine.

Unfiltered listing retains the existing metadata-only private-record contract.
Filtered search returns public items only. New private revisions never replace
accepted search inputs. Legacy records without a publication snapshot have
only their immutable title and author fields indexed; abstract/MSC coverage
requires a separately validated historical-publication backfill. Search does
not yet federate arXiv, Crossref or OpenAlex, search PDF bodies, or provide
semantic/vector ranking.

## Review reports

`GET /api/v1/submissions/{id}/review` downloads `mathesis-review.json` with
`schema_version: 1`, `service: "Orchestra"`, current source SHA-256, paper
version, claims revision, submission status, claims, review rounds and letters.
Each historical round retains its own version and claims revision; consumers
must not interpret an old round as a review of the current source.

Both authors and staff get the existing **author-safe** projection from this
export. Advice, letter drafts, provider task identifiers, raw model prompts,
credentials and private probe files are excluded. Private reports never enter
the public search index. Reports use `Cache-Control: private, no-store`.
Review findings are judgements; they never confer a Lean verification badge.
The underlying policy and review-generation pipeline are unchanged.

## Lean intake and verification

`POST /api/v1/formalizations` takes JSON with `title`, `authors`, `statement`,
`source_reference`, `toolchain`, `license` and `solution`. A source is a UTF-8
Lean module of at most 1 MiB. A repository permalink at a fixed revision is
recommended for attribution; a URL is a reported source, not an independently
verified provenance claim. The uploader's identity and original authors are
stored separately. The deposit is private to its owner and staff.

The complete validated request and authenticated owner determine a stable
content id. Exact retries return the existing deposit. Metadata is immutable;
changed source/metadata yields a new deposit. The source lives in GridFS.
`GET /formalizations` lists the owner's deposits; `GET /formalizations/{id}`
returns metadata and linked attempts; `/source` downloads the original module.
Deposit listing uses stable descending content ids, not submission-date order.

`POST /formalizations/{id}/attempts` with `{record, claim}` is an explicit
verification request. Only the deposit owner can request it. The stored
`toolchain` must exactly match the confirmed target. The source enters the
existing target → attempt → independent verifier path, with all its quotas,
idempotency, current-input checks, receipts and public-proof rules. Attempt ids
are appended to the deposit using revision fences. A retry after a crash between
attempt admission and link persistence finds the same attempt and repairs the
link. The attempt reconciler recovers admitted verification work.

A deposit is **received**, not verified. A verified attempt becomes public only
while its source and exact confirmed target remain current and public. A
standalone deposit does not create a paper, target or approval.

### Multi-file formalizations and provenance

Optional `modules` maps flat capitalized names (e.g. `Helpers`) to complete Lean
source. Up to 32 supporting modules are accepted; main plus supporting sources
must total at most 1 MiB. Each file is stored in GridFS with its digest. Optional
`formalizers` separates Lean contributors from original research authors;
`source_revision` records a full 40/64-digit Git commit. A reported repository
or commit is provenance supplied by the uploader, not a fetched/authenticated
repository snapshot. `GET /formalizations/{id}/project` downloads all sources
and attribution as reusable JSON; it is owner/staff-only and no-store.

The independent verifier checks every supporting source, rejects cycles,
path-like/reserved/case-colliding module names and trusted-library shadowing,
and builds modules in dependency order between Target and Solution. It checks
the final theorem's type and axiom closure in a separate Check module. The
same conservative forbidden-construct rules apply to every file, including
unused modules. Receipt `solution_digest` is SHA-256 of the UTF-8 source for
legacy single files; with modules it hashes the compact JSON tuple
`["mathesis-project-v1", solution, modules]` with module keys sorted. Replay,
quotas, leased jobs, current-input checks and credit all use that bound digest.

Supported imports are Mathlib, Target and the submitted flat modules. This is
a bounded source-project format, not execution of arbitrary Lake projects:
custom packages, nested module paths, user build scripts, network installs and
unsupported Lean metaprogramming are not run. Dependencies come from the trusted
pinned Lean/Mathlib workspace. Only a verifier receipt confers Lean ✓.

The verify request accepts `expected_target` (SHA-256). The target-aware web
flow supplies it and refuses a changed target. Original authors, formalizers,
source revision and license remain immutable on the attempt and appear publicly
with verified proofs. Submitting identity and leaderboard credit remain separate
from reported authorship; authorship is not independently authenticated.

### Formalization participation directory

`GET /formalizations/opportunities?q=&kind=&status=&limit=&before=` is public.
It returns current author-confirmed targets for paper results and conjectures,
with status `open`, `verified` or `disproved`, toolchain/digest, public verified
counts and winner attribution/receipt. It never exposes queued/rejected private
attempts or standalone private deposits. Search matches all case-insensitive
words across record, title, statement label, statement and author names.

Mongo uses the `solve_files` admitted/id index and cursor batches. Domain
projection rechecks live visibility, admission, current version, claim revision
and target confirmation before filtering and pagination. It stops after one
extra matching result, rather than materializing all records in memory. Very
selective queries can still scan multiple batches; this is not full-text or
semantic search. Cursors use descending solve-file ids, not submission dates.

The website exposes this directory in primary navigation. A contribution link
opens the Lean form with statement, research authors, source link, pinned
toolchain and target digest already selected. Users explicitly submit for
verification; unlinked deposits remain private. Existing rejected/queued
attempts and new attempts can be inspected from the depositor's workspace.

## NyxID connection

The machine-readable research-service contract is
`GET /api/v1/openapi.json`. Paths contain the complete `/api/v1` prefix so
NyxID's parser resolves them against the **venue origin**. This spec covers the
research-service operations listed in it, not every staff/admin operation.

An operator connects the venue origin in NyxID, associates this OpenAPI URL,
and enables the existing caller access-token forwarding mode. Mathesis verifies
NyxID access tokens using its existing JWKS/audience rules and evaluates the
caller in Layer 2. Never register a shared owner's personal token as a general
multi-user credential: that would collapse callers into one person. Plain
identity headers are not accepted as authentication.

NyxID only forwards an actual request bearer in this mode. API-key-only and
session-only MCP calls do not synthesize a bearer; such deployments need their
existing supported delegation integration before private Mathesis operations
can work. This change does not silently trust injected headers or provision
new upstream access. A spec being discoverable is not proof of authentication
working: validate identity with `/api/v1/me` before using private resources.

For a connected service, use the installed and signed-in NyxID CLI:

```sh
nyxid proxy discover
nyxid proxy request mathesis api/v1/me
export WISHPOOL_NYXID_SERVICE=mathesis
mathesis-contribute papers 'prime gaps'
mathesis-contribute review <submission-id>
claude mcp add mathesis -e WISHPOOL_NYXID_SERVICE=mathesis -- mathesis-contribute mcp
```

The SDK invokes NyxID directly without a shell; JSON bodies use stdin and no
bearer is passed on the command line. A nonzero proxy exit fails the tool; no
private response is fabricated. Direct mode remains available with
`WISHPOOL_URL` and optional `WISHPOOL_TOKEN`; anonymous access works for public
research discovery. Production service registration and live NyxID MCP
identity forwarding are separate operational checks, not completed by a local
unit test.

## Further foundation work

The next substantial capabilities are federated literature discovery with
source-specific attribution and cached fetch timestamps; historical archive
backfill; general Lake-project import with pinned custom dependencies; and
an author-facing research job that adapts deposited proofs to confirmed targets.
The existing review pipeline supplies literature analysis, but it is not a
standalone bibliographic index. Publication still follows admission; separating
archive deposit from review acceptance is a distinct policy/data-model change.
