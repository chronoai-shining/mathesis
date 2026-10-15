# Coverage-certificate consumption boundary

P2-C's downstream handling is explicit and fail-closed. **No coverage-certificate
protocol or checker is admitted by this Mathesis build.** The application exposes
that fact at `GET /api/v1/knowledge/capabilities`, in CLI/MCP
`knowledge-capabilities`, and in the private review dossier/Markdown. A stored
certificate claim remains an uncertified claim. It cannot replace independent
source-audited qualification in publication admission.

This statement is scoped to the contracts and publications actually admitted by
Mathesis. It does not assert that no related checker exists anywhere upstream.

## Inspected evidence

The consumed publication is bound to:

- Trureturing source `e7dcc89974fd44cabd0f10781eccc135eddf8d1c`;
- source tree `442f387c1013f60d23f70faa1927ea29fda57d21`;
- bundle `sha256:6c92665f572be41df6f8fad0d246f4e141e9cf036196608239792f787121ec26`;
- downstream index `sha256:2838a557bfddda5a1f9410e0dcf596a67112c0ec0f163c4cf1a787abcce15d2b`.

The exact upstream [TruthExportSchema.cs](https://github.com/the-omega-institute/trureturing/blob/e7dcc89974fd44cabd0f10781eccc135eddf8d1c/tools/Trureturing.Truth/TruthExport/TruthExportSchema.cs)
was checked against Git blob `8c7954ece9491000bff2e4adbb7d0df7453aee7d`.
Its `stratalint.truth-export.v2` nodes contain repository path, frozen-node content
address, node axiom closure, declaration identities, prerequisite node identities
and separate `freeze_status`. It has no supplied Mathesis target, parameter map,
coverage-operation witness or coverage-checker receipt for a downstream claim.
The schema explicitly separates frozen membership from proof and content identity.

The corresponding [TruthExportCommand.cs](https://github.com/the-omega-institute/trureturing/blob/e7dcc89974fd44cabd0f10781eccc135eddf8d1c/tools/StrataLint.Cli/Commands/TruthExportCommand.cs)
was checked against Git blob `5a284b408d3c304276f0e67c5563b7e3040c00da`.
It projects Closed nodes from one immutable revision and an explicit validated
Lean report, and validates frozen state records against that report. This is
valuable published source/proof-state evidence. It is not an admission operation
for a novel downstream natural-language or author-confirmed target.

The already acquired [report publication](https://github.com/the-omega-institute/trureturing/releases/tag/lean-cache-verify-v1-db584cd6d46c92f209a44c0f1c829460d327499d-linux-arm64-report-stratalint-raw-lean-report-v3-env-43c54620c14f-ci-38727419398-1)
retains source/report provenance and material digests. Its canonical type material
supports declaration identity and inspection. Matching a type hash or checking
an arbitrary Lean proof does not certify that only an admitted finite set of
binding operations derives the exact downstream target from the named baseline.
See `KNOWLEDGE_CONSUMPTION_VALIDATION_2026-10-16.md` for acquisition receipts.

## What would admit a real upgrade

A usable upstream publication/service must supply an explicitly recognized
protocol and a replayable, attributable verification artifact binding all of:

1. The author's confirmed exact target digest and its correspondence scope.
2. The complete joint prerequisite identities; no dropped premise or hidden
   imported helper, and no manuscript-derived result used as its own authority.
3. Immutable source snapshot and index revision.
4. The finite permitted operations and their actual witness.
5. Checker protocol/version, toolchain and verification authority.
6. The verification artifact's exact bytes/digest and validity/revocation scope.

The downstream consumer must reject a mismatch in any coordinate before storing
recognized certificate evidence. Until that actual interface is available,
Mathesis preserves source-audited comparisons at their existing level. No local
protocol string, operator flag, model assertion, ordinary Lean success, finite
computation receipt, upstream merge or Frozen label enables an upgrade.

## Validation and remaining acceptance gate

Existing domain regressions reject replacing source-audited covered relations
with ordinary Lean certificate claims. The added application regression retains
a real saved query receipt while refusing admission for claimed ordinary Lean,
truth-export and unknown certificate protocols. HTTP checks verify that the
capability endpoint reports no recognized protocol and no ordinary-Lean coverage
upgrade. The original evidence and qualification remain readable.

The P2-C acceptance item “one real L2 → L3 upgrade” remains **not achieved**:
the admitted artifact contracts do not contain the required target-specific
certificate interface. No upstream code, CI, judge rules, publications or service
contracts were changed to manufacture that capability. All feasible current
downstream handling is implemented; consuming a real compatible certificate
awaits that specific external capability.

Local validation on this implementation: **359 API tests** (real task-owned
Mongo/GridFS and TeX), **325 Web tests** and **3 SDK tests** passed. Strict API/SDK
Clippy, formatting, web type/lint/build checks and dependency checks passed.
The applicable original PR CI remains the final merge gate.
