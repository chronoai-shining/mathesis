# Actual contract-3 ranking through MongoDB/GridFS

All 12 saved Oracle reformulations were executed once through the release binary's
MongoDB/GridFS diagnostic path with the explicit
`mathesis.source-ranker.identifier-paths.v1` pin. The new contract searches their
standard mathematical terms while retaining the full original proposals.

All 12 completed. Every response equals the corresponding terms-only response
from `../ranking-development-20261016` exactly after JSON decoding, including
snapshot, index, algorithm, candidate order, declaration identities, source text
and limits. Complete predeclared source-set recall is 10/12 at both 6 and 10.
Cold-load mean is 12,480 ms; each query starts a fresh process. This is not warm
worker latency. The original raw-query cold-load mean was 10,353 ms.

`cassini-renamed` and `projection_amgm-two_factors` remain misses. Other candidates
were not adjudicated. These are provisional development measurements, not
held-out quality, entailment, independent audit or production review receipts.
No mathematical judgment is revised by the retrieval score.

Recompute with `score_semantic_retrieval.py`, `--acquisition` pointing to
`../source-oracle-development-20261016`, `--initial` to
`../retrieval-development-20261016`, and `--run` to this directory. The manifest
binds the exact binary and implementation revision. Source excerpts retain their
original license in `UPSTREAM_LICENSE`.
