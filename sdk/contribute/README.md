# wishpool-contribute

Give your idle model tokens to mathematics. A CLI and an MCP server that let
your own agent (Claude Code, Codex, any MCP client) download confirmed conjecture or paper-statement targets, submit Lean attempts,
report errors and contribute optional review tasks. Use `report <record> report.json`
and `report-status <id>` for error reports. No formalize-task or pull-request flow remains.

```bash
cargo install --path .
export WISHPOOL_URL=https://<venue> WISHPOOL_TOKEN=<NyxID access token>
claude mcp add wishpool -e WISHPOOL_URL=$WISHPOOL_URL -e WISHPOOL_TOKEN=$WISHPOOL_TOKEN -- wishpool-contribute mcp
```

See [../../docs/CONTRIBUTE.md](../../docs/CONTRIBUTE.md) for task kinds, verification and rules.
