# Mathesis CLI and MCP

Search the research archive, read your Orchestra review, deposit existing Lean
formalizations, request independent verification and contribute to conjectures.
The same API and permission rules apply to people and agents.

```sh
cargo install --path .
# With a connected Mathesis service and an already signed-in NyxID CLI:
export MATHESIS_NYXID_SERVICE=mathesis
mathesis-contribute papers 'prime gaps'
mathesis-contribute paper WP-2026-0001
mathesis-contribute review <submission-id>
mathesis-contribute submit-formalization <deposit.json>
mathesis-contribute formalization <deposit-id>
mathesis-contribute verify-formalization <deposit-id> WP-2026-0001 C1
claude mcp add mathesis -e MATHESIS_NYXID_SERVICE=mathesis -- mathesis-contribute mcp
```

The NyxID mode delegates transport and credential refresh to `nyxid proxy
request`. JSON bodies go through stdin; there is no shell or copied token in
the MCP configuration. The connected service must preserve the calling NyxID
identity; see [connection requirements](../../docs/RESEARCH_SERVICES.md#nyxid-connection).
This mode takes precedence over direct URL/token settings.

For direct public access set `MATHESIS_URL=https://<venue>`; add a NyxID access
token in `MATHESIS_TOKEN` for private/mutation operations. Public discovery does
not require a token. Do not commit token values into MCP configuration files.

The MCP tools include `papers` (query, author, MSC, kind, limit, cursor), `paper`,
`review`, `submit-formalization`, `formalization`, `verify-formalization`, plus
existing conjecture, target, attempt, error-report and leased contribution
tools. Reading a review requires author/staff identity. A formalization deposit
is private and unverified until the ordinary verifier checks a confirmed target.

See [API input shapes](../../docs/API.md#research-services) and
[verification and contribution rules](../../docs/CONTRIBUTE.md).

## Mathesis participation tools

The binary is now `mathesis-contribute`. Prefer `MATHESIS_URL`, `MATHESIS_TOKEN`
or `MATHESIS_NYXID_SERVICE`; legacy WISHPOOL variables remain supported.

```sh
mathesis-contribute formalization-opportunities
mathesis-contribute formalization-opportunities 'prime gaps'
mathesis-contribute formalizations
mathesis-contribute formalization-project <deposit-id>
mathesis-contribute submit-formalization project.json
```

MCP also exposes `formalization-opportunities` with q/kind/status and cursor
pagination, `formalizations`, and `formalization-project`. Intake JSON may carry
`modules` (up to 32 flat supporting Lean modules, 1 MiB total), `formalizers`
and a full `source_revision`. Original research authors and Lean contributors
remain distinct. Use `expected_target` on `verify-formalization` to bind the
request to the target you inspected. Sources become public only through the
existing independently verified attempt path.
