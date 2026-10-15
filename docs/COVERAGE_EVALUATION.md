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

## Measured development baseline, 16 October 2026

The retained [run](../api/evaluation/runs/oracle-development-20261016/run.json)
and [score](../api/evaluation/runs/oracle-development-20261016/score.json) measure
the contract-3 conjecture-referee prompt at `cbc9b7c6da9e82eb492409168e52836c52349471`.
Each case is submitted alone as a statement; this is neither a manuscript
review nor a CMA source audit. Requests, streams, extracted answers and receipts
are retained alongside the run. The observed model was `gpt_6`, effort `pro`.
No held-out case was requested.

| Observation | Measured result |
| --- | --- |
| Completed usable answers | 11 / 12; one uncertain transport failure retained |
| Covered-label recall | 5 / 6, with missing acquisition in the denominator |
| Refuted-label recall | 3 / 6 |
| Relation agreement over all cases | 8 / 12 |
| Complete required premise retrieval | 0 / 6; no actual knowledge adapter in measured revision |
| Abstentions | 2 / 11 observed answers |
| Refuted claim reported open with content | 1: `projection_amgm-original` |
| Mean elapsed time | 439,742 ms over 11 completed calls |
| Monetary cost | Unmeasured; model token estimates are not billed cost |

The original projection-matrix inequality has an exact rational counterexample
in the frozen label evidence, but Oracle reported it open and content-bearing.
The renamed version was unclear. This is a concrete development failure to
address through source retrieval, independent comparison and bounded arithmetic
replay. The two-factor scope-change negative was correctly rejected. The
`cassini-renamed` stream ended unexpectedly without a task identity; it was not
blindly resubmitted and remains missing.

The scorer now also reports refutation recall and false open readings explicitly;
the original positive-coverage metric alone did not describe that failure.
This is additive reporting of the same frozen corpus and saved answers, not a
changed label, held-out result or post-hoc acceptance threshold. Its regression
suite has 18 tests. All labels remain provisional source-based transcriptions,
with written arguments and exact checks, not external human adjudication.

Reproduce normalization and scoring without any provider call:

```sh
python3 api/evaluation/normalize_oracle.py \
  api/evaluation/runs/oracle-development-20261016/acquisition \
  /tmp/mathesis-reproduced-oracle-run.json
```

The output path must be new; historical runs are never overwritten. This
baseline establishes a measured starting point. It does not satisfy the full
live Oracle/CMA acceptance test or issue #4's human-reviewed evaluation gate.

## Actual raw source retrieval, 16 October 2026

The [retained diagnostic run](../api/evaluation/runs/retrieval-development-20261016/README.md)
queries all 12 development inputs against the immutable full source index in
Mongo/GridFS. Requests use only the confirmed statement and the `Question`
label. All queries completed. Complete predeclared source-set recall was 0/12
at both 6 and 10 candidates. This finite exact-source metric does not determine
whether a different returned theorem could support a valid comparison.

Release cold-load latency averaged 10,353 ms, including index reconstruction
for each process. Warm worker latency is unmeasured. Debug cold queries reached
the separate diagnostic 180-second bound; these do not establish production
latency or justify changing shared infrastructure or the worker's limits.

`retrieval_eval.py` retains each request before execution, exact response hashes,
source/index identities, limits and per-case timings. Its scorer distinguishes
module discovery from matching every required declaration and keeps failed
queries in the denominator. Rerunning its `score` command performs no queries.

`source_referee_eval.py` prepares all prompts from these actual results and
acquires one streaming Oracle call per development case, with two calls active.
It includes at most the first six source candidates under the worker's prefix
bound, requests the contract-2 reformulation schema, and retains every request,
stream and observed provider receipt. Original no-source exports were checked
against all 12 saved baseline requests and remained identical. No held-out case
is admitted by these tools. An uncertain streaming call is never resubmitted.

`semantic_retrieval_eval.py` executes usable saved Oracle proposals on the same
index and retains intent before each query. The mathematical fields and UTF-8
limits match the worker's proposal contract; reason stays outside the ranking
query. Existing intents are never overwritten. `normalize_source_oracle.py`
uses actual retrieval records, not a model's assertion that it searched, to
report query status. These diagnostic artifacts cannot enter production as
review receipts or upgrade Oracle output to independently audited coverage.

The source-informed Oracle acquisition is still running as this increment is
prepared. It is not a completed source-audit or quality improvement claim.


## Complete reformulation/ranking development comparison

The source-informed Oracle acquisition completed all 12 original streaming calls
with observed `gpt_6` / `pro`. All requests, streams, extracted answers and
receipts are retained in `api/evaluation/runs/source-oracle-development-20261016`.
It has coverage recall 6/6, refutation recall 3/6, relation agreement 9/12 and
one unsupported open/content reading. The 11 cases usable in both Oracle runs
have identical normalized relations. The apparent +1 agreement and +1 coverage
come from obtaining the previously missing Cassini answer, not a measured gain
on the common cases. The projection AM–GM original remains falsely open/content;
the renamed projection and signed-weight Bonferroni cases remain unknown.
These are statement-only readings, not independent audits or full manuscripts.

The original 12 raw queries, 12 actual full-field reformulations and 12 actual
ranked-term queries all completed on the same MongoDB/GridFS source index. The
last run uses the explicit new ranker pin. Every returned JSON result matches
the corresponding direct-index experiment exactly, including sources, identities,
ordering, excerpt/declaration truncation and limits.

| Method | Complete predeclared declarations @6 | @10 |
| --- | --- | --- |
| Raw statement, legacy ranker | 0/12 | 0/12 |
| Full reformulation, legacy ranker | 3/12 | 3/12 |
| Terms only, legacy ranker | 7/12 | 7/12 |
| Terms only, identifier components | 7/12 | 8/12 |
| Terms only, identifier components plus module paths | 10/12 | 10/12 |

The selected method still misses the predeclared `cassini-renamed` and
`projection_amgm-two_factors` sources; this does not adjudicate alternative
returned theorems. The full-field/identifier-path ablation yields 6/12. All
counts require the complete declaration set, not merely a module hit. Labels
remain provisional, all tuning uses development only, and holdout is unqueried.
No audited correctness/coverage gain is claimed from these retrieval counts.

Release cold-load mean through Mongo/GridFS is 12,480 ms for the selected method,
versus 10,353 ms for the original raw queries. Direct local-index construction
was 8,872 ms; warm terms-only searches averaged 10.5 ms (range 8–13), excluding
Mongo/GridFS/network/provider latency. Runtime warm latency is not measured. These single runs used a shared development machine; they are not an isolated performance benchmark.
The earlier debug diagnostic timed out on all 12 cold loads at 180 seconds;
those failures remain in the task evidence directory and do not replace the
release measurements. No infrastructure or runner change was made.

Reproduction tools are `ranking_eval.py verify` (exact compressed outputs and
query derivations), `normalize_source_oracle.py` (provider streams), and
`score_semantic_retrieval.py` (actual full-field or ranked-term queries).
The four corresponding run directories preserve original upstream source licenses.
New review rounds pin knowledge contract 3; contracts 0/1/2 retain their prior
behavior. Independent managed CMA review, human adjudication, held-out quality,
full manuscript evidence, cost and production deployment remain unmeasured.
