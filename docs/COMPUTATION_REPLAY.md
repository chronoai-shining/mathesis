# Bounded exact computation evidence

Authors can request an independent finite calculation for a declared evidence
attachment. Mathesis executes a fixed arithmetic checker on JSON data. It never
runs uploaded programs, scripts, Lean modules or arbitrary expressions through
this mechanism. The original source and attachment bytes remain in GridFS.

The first supported protocol is `mathesis.rational-projection-amgm.v1`. Its input
contains three nonzero integer vectors of dimension 1–6 and an integer witness of
the same dimension, all entries between −100 and 100. The file is at most 16 KiB;
unknown fields, floats, invalid dimensions and unknown protocols are refused.
Arithmetic uses checked exact rational operations. Representational overflow
returns `arithmetic_limit`, with no invented or rounded answer.

For each vector vᵢ the checker reconstructs Pᵢ = vᵢvᵢᵀ/(vᵢᵀvᵢ), rechecks symmetry
and idempotence, and computes xᵀDx where

D = ((P₁ + P₂ + P₃)/3)³ − (P₁P₂P₃ + P₁P₃P₂ + P₂P₁P₃ + P₂P₃P₁ + P₃P₁P₂ + P₃P₂P₁)/6.

These are positive semidefinite rank-one orthogonal projections by their stated
construction. Negative xᵀDx refutes positive semidefiniteness of this D. A
nonnegative witness does not establish a universal matrix inequality. The
checker does not determine whether this assertion matches the submitted claim;
that correspondence remains an explicit source-audit obligation.

Place this in `projection.json` inside the uploaded source archive:

```json
{
  "protocol": "mathesis.rational-projection-amgm.v1",
  "vectors": [[1, -1, -1], [-1, 1, -1], [-1, -1, 1]],
  "witness": [1, 1, 1]
}
```

Request replay in the existing `mathesis-evidence.json` manifest:

```json
{
  "schema_version": 1,
  "files": [{
    "path": "projection.json",
    "kind": "computation",
    "description": "Exact rank-one projection counterexample",
    "claims": ["C1"],
    "replay_protocol": "mathesis.rational-projection-amgm.v1"
  }]
}
```

This example gives **−8/243**. Claim references in the manifest are author
assertions, not automatically checked correspondence. Existing attachments
without `replay_protocol` retain their inventory/read-only evidence treatment.
Ordinary Lean solving continues through the author-confirmed target, attempt
and independent verifier path; finite arithmetic creates no solving credit.

New rounds pin `computation_contract: 1`. Requested replay occurs before Oracle
admission. Receipts bind the exact attachment inventory/hash, manuscript source
hash, version, claims revision, round and checker protocol; they are immutable
and saved under the referee lease and aggregate revision. Interrupted retries
reuse the saved receipt. Changed input identities and late writes are refused.
Old rounds default to contract 0 and receive no retrospective calculations or
new prompt contents. No database backfill is required.

Oracle, source audit, advice and private letter receive the same saved result
with its finite scope. A calculation never directly changes correctness,
coverage or publication policy. Author/co-author/staff referee views and review
JSON/Markdown expose the receipt. Public paper details do not expose it. The
existing SDK/CLI/MCP review export carries the same JSON fields; there is no
client endpoint for submitting a successful checker receipt.

Reproduce the checker independently of any manuscript or provider:

```sh
cargo build --manifest-path api/Cargo.toml -p mathesis-review --example replay_computation
api/target/debug/examples/replay_computation < projection.json
python3 api/evaluation/check_computation_replay.py api/target/debug/examples/replay_computation
```

The last command compares 61 deterministic cases with a separate Python exact
fraction implementation using rank-one dot-product contractions, including all
27 ordered terms of the mean cube and all six permutation terms. It is a
cross-check of finite arithmetic, not a source-audit or coverage certificate.
