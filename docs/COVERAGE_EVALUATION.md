# Coverage evaluation protocol

The initial corpus contains 18 statement-level cases from six source families:
pigeonhole fibers, Cassini–Fricke recurrences, projection AM–GM, capture-count
Bonferroni bounds, pure-state matrices and weighted Turan inequalities. Twelve
development cases and six held-out cases are separated by complete theorem
family. This is a small authored engineering corpus, not the proposed 120-case
benchmark and not a representative sample of mathematical research.

The source is Trureturing commit
`e7dcc89974fd44cabd0f10781eccc135eddf8d1c`, transported in the existing Pages
source publication
`sha256:6c92665f572be41df6f8fad0d246f4e141e9cf036196608239792f787121ec26`.
Every copied source file matches the source hash in the original inspector
report. The original license accompanies the source copies. Source module,
declaration name key, statement id, frozen-node id and source hash remain
separate identities. This corpus does not equate a successful upstream CI run
with a new Mathesis verifier receipt.

## Labels and scope

Each label includes a written substitution argument or counterexample and exact
source identities. Negative examples have independent exact rational or integer
checks, including the projection quadratic form `-8/243`, failure of the signed
capture bound, and failure of Turan outside its tail hypothesis. The two-projection
identity is expanded as a noncommutative polynomial, preserving word order.
These checks establish their specific computations; they are not Lean receipts,
binding certificates or evaluations of literature priority.

Labels are explicitly **provisional agent transcriptions and written checks**.
No human adjudication or independent expert participation is claimed. The
projection case labels the exact formal assertion's refutation; the source's
bibliographic description is not independently established by this dataset.
New-result, priority and sparse-field cases requiring expert evidence remain a
separate acquisition task. Do not fill their place with invented novelty labels.

Case inputs are independent, have no proofs submitted, and are screened as
questions/conjectures for the statement-only baseline. This is not a simulated
paper review. A subsequent full review must use genuine manuscript/proof inputs
and the complete Oracle → CMA → domain-policy path.

## Freeze and holdout

`api/evaluation/coverage_v1/freeze.json` commits the exact corpus, source
identities, source bytes and family allocation. Its SHA-256 is
`4cbc93154e116f57923b3cf475904638640856a92e929573670b6a60c8bf884b`.
Changing a frozen file invalidates the run. Corrections require a new corpus
version with the reason recorded; never silently relabel an evaluated result.

The evaluator refuses held-out input export or scoring without a preregistration
that references this freeze, a nonempty recorded development baseline, fixed
targets and candidate revision. This prevents accidental use, not an operator
with filesystem access deliberately bypassing the protocol. The corpus authors
have seen the authored examples. The holdout claim is that those families have
not been used for model queries, retrieval tuning or prompt adjustment.

Preregistrations use `mathesis.evaluation-preregistration.v1` with
`freeze_sha256`, `recorded_at_utc`, `candidate_revision`, `targets`,
`development_baseline_file` (relative to the registration) and its
`development_baseline_sha256`. Targets are set after the development baseline
and before held-out predictions. Changing targets after opening the holdout
requires a new independent test set; an extra run does not restore blindness.

## Reproduce

From the repository root:

```sh
python3 api/evaluation/coverage_eval.py validate
python3 -m unittest discover -s api/evaluation -v
python3 api/evaluation/coverage_eval.py inputs > /tmp/mathesis-development-inputs.json
python3 api/evaluation/coverage_eval.py score --run /path/to/recorded-run.json
```

`inputs` exports only case id, statement and input kind; no labels, expected
declarations or source evidence are sent to the model. The
`mathesis-review/examples/evaluation_prompt.rs` program accepts one such case
on stdin and produces a streaming Oracle request using the actual contract-3
conjecture-referee prompt. It performs no network I/O. Provider acquisition must
retain the exact request, answer, task id, observed model, time and uncertainty
about cost. Missing cost is `null`, never zero. Do not retry an uncertain submitted
request as a fresh run merely because it is slow.

Recorded runs use `mathesis.coverage-evaluation-run.v1`, the freeze hash,
`split`, `implementation_revision`, `method`, `model`, `snapshot`, `budget`,
`started_at_utc`, and `predictions`. Each prediction has:

- `id`, `relation` (`covered`, `refuted`, `candidate`, `unknown`, `uncovered`);
- `query_status` (`complete`, `partial`, `unavailable`, `timeout`,
  `invalid_artifact`, `not_configured`), and an explicit `novelty_claim` boolean;
- `returned_sources`, each with `family`, `statement_id` and
  `declaration_name_key`, as observed by retrieval;
- measured `latency_ms` and `cost_usd`, or `null` if unmeasured.

The system query status comes from actual calls. An Oracle narrative mentioning
Trureturing does not change `not_configured` into `complete`. Keep raw model
readings and the normalization rule alongside every run. Model assertions remain
assertions; this offline scorer grants no domain evidence qualification.

## Metrics and gates

The scorer reports positive coverage recall, retrieval of the complete required
premise set, known results incorrectly called uncovered, asserted relations
contradicting the provisional labels, abstention, query status counts, per-domain
readings, observed latency and partially observed cost. Each ratio retains its
numerator and denominator. No observations is not a perfect score. Missing
predictions remain in completion and positive recall denominators. A partial
search can establish a specific relation; the two dimensions are scored
independently. Retrieving one member of a joint premise set does not count as
retrieving the complete justification.

No case certifies global novelty. Accordingly every affirmative novelty claim
requires investigation, and an incomplete mandatory search supporting such a
claim is a separate protocol violation. No-match and abstention are not automatic
novelty findings. Protocol regressions (wrong version, fabricated receipt,
corrupted source, private-data leakage, source revocation and replay) also need
domain/storage/adapter tests; a numerical benchmark does not replace them.

The 17 initial evaluator and arithmetic tests check scoring integrity and label
calculations only. They are not a measured model baseline. Live Oracle-only
observations, full Oracle/CMA reviews, artifact retrieval measurements and
external expert adjudication must each retain their own evidence and limits.
