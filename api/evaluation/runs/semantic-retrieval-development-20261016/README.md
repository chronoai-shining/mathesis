# Actual contract-2 reformulation queries

All 12 saved, usable Oracle proposals were executed once against the same
MongoDB/GridFS index as the initial raw query run. These are full-field lexical
queries using the preserved contract-2 binary; they do not use the new ranker.
Every admitted request and completed response is retained with checksums.

The complete predeclared source set appears at rank 6 or better for 3/12 cases.
These are candidate-retrieval measurements, not revisions to Oracle's original
judgments or independent source audits. Labels remain provisional. No held-out
case was queried. Latencies include a cold Mongo/GridFS load per query.

Recompute with `score_semantic_retrieval.py`, pointing `--acquisition` to
`../source-oracle-development-20261016`, `--initial` to
`../retrieval-development-20261016`, and `--run` to this folder.
Original upstream source excerpts retain their license in `UPSTREAM_LICENSE`.
