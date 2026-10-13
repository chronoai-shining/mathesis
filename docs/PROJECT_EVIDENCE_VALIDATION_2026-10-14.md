# Project and evidence intake validation — 2026-10-14

This extends the [2026-10-13 stress test](MATHESIS_STRESS_TEST_2026-10-13.md).
All writes were private fixtures in the existing isolated local MongoDB database.
Research repositories were read only. No production publication or new live
Oracle/CMA review was started.

## Real project intake

Used `repos/klein-four-lazy-ca-supplement`, including `Audit.lean`,
`KleinFour.lean` and its nested `KleinFour/` sources: **81 Lean modules,
533,810 source bytes**. Preserved `lean-toolchain`, `lake-manifest.json`,
`lakefile.toml`, `README.md` and `LICENSE`.

- Full JSON project was admitted and exported; every module and configuration
  string matched the input. Another user received 404 on export.
- Browser ZIP intake read all 81 modules and five configuration/documentation
  files, inferred the Lean/Mathlib pin, displayed preflight and saved the source.
- Preflight reported 79 compatibility findings, including the absent target
  adapter, audit commands and restricted `set_option` constructs. It did not
  claim that the full research project had been verified. This is a successful
  source-intake test, not an independent verification of that entire library.
- No attempt or public proof was fabricated for this project.

## Independent Lean execution

Ran the optional real checker regression against a fresh copied workspace with
`leanprover/lean4:v4.33.0`, Mathlib
`db584cd6d46c92f209a44c0f1c829460d327499d`.
Memory and active Lean/Lake executables were checked before execution; the two
cases ran sequentially. The original prepared workspace was not modified.

The project imported `Research.Helpers` from `Research/Helpers.lean`, used a
`namespace Research` block and imported `Mathlib.Data.Nat.Basic`. A separate
root adapter referenced `Research.helper`:

- target `True`: **Proved**;
- target `False` with the same proof: **Rejected**.

The checker test asserted the complete project digest in both receipts and
completed in **496.56 seconds**. A separate filesystem regression rejects
symlinked output parents before artifact cleanup and confirms that the original
source outside the temporary workspace remains unchanged. This tests nested paths, namespace support,
prepared dependency resolution and the exact-target boundary together.

## Real computational evidence

Bundled `petersen-zero-forcing/notes/k4-note-v2.tex` with three original files:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `data/forcing-witnesses.json` | 29,653 | `0c000ef766154da8e5b61a792c209c476afd00d38cf34ff3883139b74ce25ef1` |
| `verify.py` | 4,886 | `80f3d5fc973eab5bb53e40a7d809778561a1a4c5c8208f05d15a1a9c3103481a` |
| `VERSIONS.json` | 1,695 | `69040450391026a6e9977ef4e52080b335d25735282bb1a80b0fb92e77979d44` |

Server inventory hashes and byte counts matched original files. Authorized
attachment downloads returned byte-identical content with private/no-store
headers; another user received 404. A manifest naming missing files returned
422. The private review JSON included the version's inventory.

A source-only revision retained the old version's three files and had an empty
new inventory. It did not inherit stale certificates. Browser intake also
uploaded the real manuscript and certificate, then displayed the saved inventory.

Tests cover binary/oversized evidence being explicitly inventory-only, SHA-256
changing with changed bytes, path rejection, duplicate intake, and transport of
non-TeX certificates through the CMA source adapter. No assertion is made that
the supplied mathematical certificate itself was executed or proved correct.

## Browser and regression checks

Used the actual **41-statement sub-quorum manuscript**. Five rows were shown per
page; a changed role and checked state survived forward/back navigation. Unit
coverage also checked all 41 rows, filtered the visible page, and confirmed that
all 41 edited rows were submitted together. Dependency choices load when opened;
page navigation returns to the confirmation controls.

The Petersen title rendered its inline formula with KaTeX. Confirmation, complete
project intake, title and evidence views were checked at **320 px and 1440 px**:
no horizontal page overflow and no browser exceptions. Preflight findings beyond
the first five are expandable. Screenshots and raw logs remain in the isolated
local stress-test artifact directory.

- Rust workspace: **263 tests passed**, with MongoDB and local TeX enabled.
  Optional real Lean execution above was run separately.
- Web: **300 tests passed**; formatting, lint, TypeScript and production build passed.
- SDK: **3 tests passed**, formatting and clippy passed.
- Rust clippy and cargo-deny passed. npm audit at the existing high-severity gate
  passed; the existing KaTeX dependency chain reports four low-severity advisories.

Regression checks also preserve the exact old referee/source prompt representation for
manuscripts without declared evidence, protecting already admitted task identities.
The source/audit adapter now transports declared evidence and reports coverage explicitly.
The separate CMA connection-readiness problem documented in the previous stress
test was not changed or represented as resolved by this work.
