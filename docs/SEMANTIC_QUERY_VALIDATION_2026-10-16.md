# Recorded mathematical query reformulation — 2026-10-16

This increment builds on merged PR #6 (`cfc332cccecc40e9ecb0686f367b34e515ecf33a`).
PR #6 passed all checks in CI `38798106776`, including real Linux Lean/image
validation. This change extends P1-C/P1-D; P1-E and P2/P3 remain open.

New rounds pin knowledge contract 2. Oracle extracts definitions, hypotheses,
quantifiers, conclusion and standard terms into one query proposal per confirmed
claim. The worker reads those proposals from its saved report, admits the whole
usable batch atomically, then executes it against the originally pinned index.
The mathematical fields form the query, and its retrieval reason is digest-bound.
Model proposals never count as executed queries.

Before filing the source audit, CMA can request one additional batch, at most
one query per claim. Admitted unfinished queries resume after lease loss. A fresh
CMA conversation uses the distinct `rN:knowledge-followup:audit` durable key,
classified as the ordinary audit kind for payer admission. A request beyond the
budget or an unusable batch removes affected sufficient comparisons; an invalid
claim identity conservatively affects all comparisons. The first requesting
answer remains in the managed transport history, while only the final audit is
filed as a completed assessment.

Layer 2 enforces current subjects, one origin and unique claims per batch,
original snapshot/index, immutable replay, per-origin round budget and live lease.
Admission additionally requires an actual Oracle reformulation receipt for each
assessed claim. Complete raw searches cannot substitute for it. Missing results
are explicit in author Markdown/UI and the model context. Source errors,
truncation and missing hypotheses retain their existing independent meanings.
Contracts 0/1 keep their old prompts and receive no retrospective semantic work.

## Verification

- API: **338 tests passed** at the final code with real task-owned MongoDB and
  TeX Live, including the Markdown/context and budget regression assertions.
  Formatting, strict Clippy and cargo-deny passed. Logs:
  `/private/tmp/mathesis-semantic-final-{tests,clippy,fmt,deny}.log`.
- Staged gitleaks scan passed with no leaks, covering 52.96 KB of changes.
- Web: 320 tests, formatting, lint, types, production/showcase builds and
  high-severity dependency audit passed; zero vulnerabilities. Log:
  `/private/tmp/mathesis-semantic-web-full.log`.
- Regression coverage includes a missing/malformed Oracle batch with otherwise
  complete raw queries, all-or-none plans, lease loss and resume, snapshot pins,
  one additional CMA batch, exhausted budgets, unassignable retrieval gaps,
  contract-1 exact Oracle prompt replay, contract-0 replay, author Markdown and
  UI limits. Managed HTTP transport tests create/replay a distinct follow-up
  audit and preserve saved account/workspace/creation keys without duplicate work.
- Local real Lean tests stay explicitly ignored; the normal Linux CI/image gate
  executes them. No upstream code or Lean workspace is executed or modified.

These orchestration tests use synthetic mathematical readings. Source searching
remains lexical matching over checked full source, with model-proposed
mathematical reformulations. This is not semantic equivalence certification or
a measured quality gain. The 6 held-out cases remain unqueried. A real company
managed CMA source audit and human-adjudicated labels remain unverified. The
previous managed sandbox is retired; no upstream sandbox was repurposed.
No production capability deployment is claimed by this increment.
