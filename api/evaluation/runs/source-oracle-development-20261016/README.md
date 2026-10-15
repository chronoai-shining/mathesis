# Source-informed Oracle development readings

All 12 frozen development cases completed one original streaming request each.
The exact initial queries, top-six source inputs, provider streams, extracted
answers, model observations and hashes are retained. No failed/uncertain request
was retried. Only the development split was queried.

The prompt exporter revision is recorded in metadata; `919a6b3` resolves to the
committed source-evaluation exporter `919a6b3ef1c5b925420035565624206ff7745d50`,
introduced before the retained retrieval run. These are statement-only Oracle readings with diagnostic source evidence
and proposed reformulations. They are not full manuscript reviews, independent
CMA audits, production receipts or certified coverage.

`run.json` normalizes the saved stream; `score.json` compares it with provisional
labels. It observes 6/6 coverage recall, 3/6 refutation recall, 9/12 relation
agreement and one unsupported open/content reading. The earlier no-source run
had one transport failure; improvement in completion must not be presented as a
controlled causal improvement in mathematical reasoning. All initial queries
missed the complete predeclared source sets. The projection AM–GM original
case remains falsely open/content despite the separate exact −8/243 witness.

Recompute using `normalize_source_oracle.py` with this directory and
`../retrieval-development-20261016`; write normalization to a fresh output path.
Newly executed reformulations are measured separately and do not retroactively
change these model judgments. Cost and independent source audit are unmeasured.

Original upstream source excerpts retain their license in `UPSTREAM_LICENSE`.
