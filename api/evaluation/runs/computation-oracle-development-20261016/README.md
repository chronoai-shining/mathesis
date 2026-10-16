# Single development diagnostic: exact projection evidence

This run selects the previously observed `projection_amgm-original` development
failure. It uses the exact saved source-informed Oracle request, inserting only
the actual finite computation attachment, replay output and diagnostic scope
before the original knowledge context. It sends one fresh streaming request.
The original statement, source candidates, source identities, methodology and
reformulation instructions remain unchanged. No expected corpus label is inserted.

The prerequest hypothesis and exact hashes are in `metadata.json`; the dispatch
record precedes the call. `projection.json` is the actual checker input and
`replay.json` is the actual Rust output. `context.json` records diagnostic hashes,
not production manuscript, round, lease or author-authorization receipts.

## Observed result

The original request reported `open` with `content`; the request with replay
reported **`known_false`** with `undetermined`. Both observed provider receipts
identify `gpt_6` with effort `pro`. The new call completed in 497,415 ms with a
complete stream, no transport error and task
`91db9180-d53b-4983-913f-af6324bf24b4`. See `comparison.json`, `receipt.json`,
`response.sse` and `answer.txt` for the exact observation.

The returned explanation correctly identifies n = N = 3, the projection
hypotheses, the six ordered products and the negative quadratic form. Its claims
of external literature inspection or model-side computation remain model
reports; the independently reproducible local finite check is recorded
separately. No manuscript PDF or independent CMA audit was supplied. This is
evidence that this request used the provided refutation, with the statistical
and execution limits below.

## Mathematical correspondence

The submitted universal assertion includes n = N = 3. Take
v₁ = (1, −1, −1), v₂ = (−1, 1, −1), v₃ = (−1, −1, 1) and x = (1, 1, 1).
Each Pᵢ = vᵢvᵢᵀ/3 is a real symmetric idempotent matrix. The vectors have squared
norm 3, mutual inner product −1 and xᵀvᵢ = −1.

For M = (P₁ + P₂ + P₃)/3, the identity v₁ + v₂ + v₃ = −x gives Mx = x/9.
Thus xᵀM³x = 1/243. For every permutation (i,j,k),
xᵀPᵢPⱼPₖx = (−1)⁴/3³ = 1/27. Therefore the average S of all six products
satisfies xᵀSx = 1/27, so xᵀ(M³ − S)x = **−8/243**. This finite witness
violates the exact universal positive-semidefiniteness assertion.

This correspondence is a written mathematical argument. The Rust checker
reproduces the finite arithmetic; neither the argument nor this diagnostic is
an independent CMA audit, a Lean receipt or a claim about literature priority.

## Reproduction and interpretation

From the repository root, inspect the retained bytes without a provider call:

```sh
python3 api/evaluation/verify_computation_diagnostic.py
```

Optionally repeat the Rust computation using a freshly built example:

```sh
cargo build --manifest-path api/Cargo.toml -p mathesis-review --example replay_computation
python3 api/evaluation/verify_computation_diagnostic.py \
  --checker api/target/debug/examples/replay_computation
```

The verifier checks the frozen development allocation, original request and
receipt hashes, unchanged prompt outside the evidence insertion, actual input
and replay hashes, an independent rational contraction calculation, complete
streams and extracted answers. It prints the two reported statuses and observed
model identities. It does not send another model request or change any corpus
labels, baseline scores or production review state.

One selected failure and one stochastic sample per prompt cannot establish
overall accuracy, isolate causation, measure repeatability, or satisfy the
full-manuscript Oracle → CMA acceptance gate. Model reformulation proposals in
the answer are not executed queries. Monetary cost is unmeasured. The six
held-out cases remain unused. This observation is kept separately from the
original 12-case baseline and is never substituted into that historical score.
