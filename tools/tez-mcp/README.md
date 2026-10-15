# tez-mcp

An [MCP](https://modelcontextprotocol.io) server that gives any MCP client (Claude Code, Claude Desktop, Cursor, an
in-house agent) the Tez compiler as four tools: `tez_check`, `tez_fmt`, `tez_run`, `tez_test`. No dependencies; it
shells out to the `tez` binary (set `TEZ_BIN`, default `target/debug/tez`, build with `cargo build -p tez-lang`).

```json
{ "mcpServers": { "tez": { "command": "node", "args": ["tools/tez-mcp/server.mjs"], "env": { "TEZ_BIN": "target/release/tez" } } } }
```

`tez_check` returns the same JSON as `tez check --json`: stable codes, spans, expected forms, counterexamples and
machine-applicable fixes, which is what lets a small model repair its own program.

Test: `node --test tools/tez-mcp/server.test.mjs`.
