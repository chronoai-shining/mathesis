# Raw source-query development run

All 12 development cases queried the immutable Mongo/GridFS index using the
release binary at `029d3d12e50698da93918c9b1f39b2c9bc64e856`. Each query was
`Question` plus the frozen statement. The diagnostic command returns ten modules;
this run scores the first six (the worker budget) and ten separately.

All 12 calls completed. None returned the complete predeclared source set in the
first ten candidates. This is exact-source recall on the authored corpus, not
proof that no returned alternative source is mathematically useful. Source
comparison and model-proposed reformulation remain necessary.

Mean elapsed time was 10,353 ms, including a fresh Mongo/GridFS load and index
construction for every process. These are cold diagnostic timings, not warm
worker latency or an end-to-end review benchmark. A separate debug-binary run
hit its 180-second diagnostic limit; it is not a production performance result.

The requests contain only the frozen statement, no label, expected source name
or declaration. Raw responses preserve source text, identities and explicit
index/result omissions. Original source licensing is retained here. Nothing in
this directory is a production review receipt, independent audit or Lean proof.
The held-out families were not queried; labels remain provisional.

Recompute without any source or provider call:

```sh
python3 api/evaluation/retrieval_eval.py score api/evaluation/runs/retrieval-development-20261016
```
