# Development ranking comparison

All 12 frozen development statements and all 12 completed saved Oracle proposals
were queried against the exact checked index. Labels and predeclared source ids
were used only in scoring, never to construct queries. No held-out case was queried.

| Ranker | Raw statement @6 | Full reformulation @6 | Terms only @6 |
| --- | --- | --- | --- |
| Legacy full-source terms | 0/12 | 3/12 | 7/12 |
| Identifier components | 0/12 | 4/12 | 7/12 |
| Identifier components plus module paths | 0/12 | 6/12 | 10/12 |

These counts require every predeclared declaration in a case, not just a matching
module. At rank 10 the components/terms cell rises to 8/12; the other cells stay
the same. `cassini-renamed` and `projection_amgm-two_factors` remain misses for
the selected method. Other returned sources may be useful; this score does not
adjudicate their mathematical relevance. Labels remain provisional.

The last method and terms-only query formulation form knowledge contract 3.
The full extraction remains hash-bound and available to the independent audit.
Exact output bytes are stored as deterministic gzip files; the response hashes
refer to the decompressed original JSON, not a regenerated serialization. Run
`python3 api/evaluation/ranking_eval.py verify api/evaluation/runs/ranking-development-20261016`
to recheck identities, query derivation and every saved score.

These direct local-index timings separate loading from warm queries and exclude
MongoDB, network and model latency. They are not production receipts or quality
measurements of a complete referee/auditor pipeline. This is a development result,
not held-out generalization or a formal coverage certificate. Original upstream
source excerpts retain their license in `UPSTREAM_LICENSE`.
