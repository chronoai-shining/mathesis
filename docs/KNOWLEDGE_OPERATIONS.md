# Importing a fixed public knowledge snapshot

Mathesis consumes the existing Trureturing Pages source projection, original
Lean-inspector sidecars and source at the same commit. An operator independently
admits the publication identity and records the source/producer CI evidence.
The importer validates byte identities and the existing format. It does not
authenticate GitHub based on a URL string or trust the manifest's own booleans.
Keep the independent source-admission evidence with the deployment record.

Prepare these local directories from existing published artifacts:

- `projection`: the existing eight-file Pages source projection, including
  `SHA256SUMS`, publication/manifest, report/provenance, snapshot, export and graph.
- `report`: original `.sha256`, `.input.attestation`, `.provenance.json` and
  `.materials.zip` alongside the raw report name. The projection supplies the
  identical raw report bytes. The ZIP is streamed and never extracted or run.
- `sources`: regular `.lean` files at their repository-relative paths from the
  exact source commit, plus its original `LICENSE` file. No upstream toolchain,
  Lake configuration, script or proof build is executed.

Create an admission JSON after independently verifying the source identity and
the applicable upstream CI/provenance contract. The SHA-256 of `LICENSE` must
match the actual source license bytes:

```json
{
  "identity": {
    "bundle_digest": "sha256:<64 lowercase hex digits>",
    "source_commit": "<40 lowercase hex digits>",
    "source_tree": "<40 lowercase hex digits>"
  },
  "admitted_by": "operator identity or audit reference",
  "source_ci_url": "https://github.com/the-omega-institute/trureturing/actions/runs/<id>",
  "report_ci_url": "https://github.com/the-omega-institute/trureturing/actions/runs/<id>",
  "protected_source_evidence": "exact observed source/branch/check evidence and retained record",
  "license_sha256": "sha256:<64 lowercase hex digits>"
}
```

With the normal MongoDB application environment configured, run:

```sh
mathesis --knowledge-import /path/projection /path/report /path/sources /path/admission.json
```

This command writes only Mathesis MongoDB/GridFS. The source index is immutable
and addressed by its content digest. The Mongo header stores the original
publication identity, admission evidence, index and all original artifact blob
references, source license, counts and admission time. Concurrent identical
imports return the existing header; uncertain outcomes never trigger eager
blob deletion. Retrying an existing import validates the index and streams a
checksum check of every retained artifact. Existing offline storage collection
recognizes these references.

The checked index has a 256 MiB total source limit, 2 MiB per source and 512 MiB
serialized storage limit. Missing or oversized source modules are explicit
omissions. Changed source bytes are rejected. Modules without exported
declarations are excluded from the candidate inventory; that inventory is not
the upstream export's full module count. Frozen and proven-not-yet-frozen
declarations retain their original status. Module imports and module frozen
prerequisites are not declaration proof dependencies.

To inspect the stored index after a process restart:

```sh
mathesis --knowledge-query 'sha256:<index revision>' 'mathematical statement or reformulation'
```

This is explicitly a diagnostic lookup, with no manuscript, review or query
receipt attached. It searches checked full source text and can use exact type
identities in the adapter API. Results retain source hashes, module/declaration
identities, declared search scope, omissions and truncation. Limits are 20
modules, 100 declarations, 80,000 excerpt bytes per source and 256,000 excerpt
bytes in total. Similarity is only a candidate-discovery mechanism. Source
identity checking does not assert that every canonical expression was decoded,
that Lean was rerun, or that a mathematical coverage relation was established.

The binary holds at most one reusable search index in its cache. Requests
already using an older immutable index may finish with their own `Arc`; a
round's durable pin must determine subsequent lookups. Mongo/GridFS remains the
authority, and no private manuscript text is inserted into this public index.

Set `WISHPOOL_KNOWLEDGE_INDEX=sha256:<exact index revision>` on the Mongo-backed
review worker after importing that index. Memory deployments cannot configure it.
New rounds pin the first admitted snapshot (including an unavailable admission)
before any Oracle request. Each confirmed claim gets a real top-six source query;
responses are stored under the existing lease and revision fences. Later process
restarts reuse completed receipts and the original prompt. A cold index load
continues in its retained cache task if a caller is cancelled; retrying does not
start duplicate rebuilds. Query and snapshot-admission deadlines are 120 and 30
seconds respectively. Source/index corruption has a separate failure state.

The prompt projection is capped at 800,000 serialized bytes including metadata;
source clipping and omitted payloads are explicit. A comparison cannot use an
omitted source or invent a receipt/declaration identity. Incomplete required
knowledge leaves publication pending; advice and a private letter still run.
After fixing configuration, availability or publication validation, use the
existing staff restart action on the same manuscript version. This creates a new
round without rewriting previous receipts or model task identities. Unavailable
sources do not trigger upstream Lean or upstream publication changes.

This increment uses raw confirmed-statement term queries against checked full
source text. Semantic reformulations, extra CMA queries and measured end-to-end
quality remain in `CAPABILITY_DELIVERY.md`; do not claim those are delivered by
an importer or query receipt. Real CMA review evidence is still required.
