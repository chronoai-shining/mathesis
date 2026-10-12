# Wishpool CLI and MCP

Search the research archive, read your Orchestra review, deposit existing Lean
formalizations, request independent verification and contribute to conjectures.
The same API and permission rules apply to people and agents.

```sh
cargo install --path .
# With a connected Wishpool service and an already signed-in NyxID CLI:
export WISHPOOL_NYXID_SERVICE=wishpool
wishpool-contribute papers 'prime gaps'
wishpool-contribute paper WP-2026-0001
wishpool-contribute review <submission-id>
wishpool-contribute submit-formalization <deposit.json>
wishpool-contribute formalization <deposit-id>
wishpool-contribute verify-formalization <deposit-id> WP-2026-0001 C1
claude mcp add wishpool -e WISHPOOL_NYXID_SERVICE=wishpool -- wishpool-contribute mcp
```

The NyxID mode delegates transport and credential refresh to `nyxid proxy
request`. JSON bodies go through stdin; there is no shell or copied token in
the MCP configuration. The connected service must preserve the calling NyxID
identity; see [connection requirements](../../docs/RESEARCH_SERVICES.md#nyxid-connection).
This mode takes precedence over direct URL/token settings.

For direct public access set `WISHPOOL_URL=https://<venue>`; add a NyxID access
token in `WISHPOOL_TOKEN` for private/mutation operations. Public discovery does
not require a token. Do not commit token values into MCP configuration files.

The MCP tools include `papers` (query, author, MSC, kind, limit, cursor), `paper`,
`review`, `submit-formalization`, `formalization`, `verify-formalization`, plus
existing conjecture, target, attempt, error-report and leased contribution
tools. Reading a review requires author/staff identity. A formalization deposit
is private and unverified until the ordinary verifier checks a confirmed target.

See [API input shapes](../../docs/API.md#research-services) and
[verification and contribution rules](../../docs/CONTRIBUTE.md).
