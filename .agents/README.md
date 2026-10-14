# .agents — shared agent environment for Tez

Single source of truth for every AI agent working on this repo, whatever tool runs it.
Project rules live in [`../AGENTS.md`](../AGENTS.md); the plan lives in
[`../docs/ROADMAP.md`](../docs/ROADMAP.md).

| Path | What | Format |
|---|---|---|
| `agents/*.md` | Specialist sub-agents (one owner per area) | Markdown + YAML frontmatter (`name`, `description`, `tools`, `model`) |
| `skills/<name>/SKILL.md` | Repeatable procedures with checklists | Markdown + frontmatter (`name`, `description`) |
| `commands/*.md` | Slash commands | Markdown prompt |
| `mcp.json` | MCP servers for this project (versions pinned) | `{"mcpServers": {...}}` |
| `link.sh` | Symlinks `.agents/` into tool-specific locations | bash |

## Using it

Tools that read `.agents/` directly need nothing. Claude Code looks in `.claude/` and
`.mcp.json`, so run once:

```bash
.agents/link.sh
```

That creates symlinks only (git-ignored). Edit files under `.agents/`, never the links.

## Agents

| Agent | Owns | Model |
|---|---|---|
| `language-designer` | `spec/`, syntax decisions, language card | opus |
| `compiler-engineer` | `crates/tez_syntax`, `tez_resolve`, `tez_types`, `tez_fmt`, `tez_cli` | sonnet |
| `verifier-engineer` | `crates/tez_verify`, `tez_solve`, refinement types | opus |
| `wasm-runtime-engineer` | `crates/tez_ir`, `tez_wasm`, `tez_web`, `runtime/` | sonnet |
| `studio-engineer` | `studio/` (React + Monaco) | sonnet |
| `dataset-engineer` | `data/` training-set pipeline | sonnet |
| `bench-engineer` | `bench/`, tokenizer measurements, TezBench | sonnet |
| `test-engineer` | fuzzing, differential and soundness tests | sonnet |
| `soundness-reviewer` | read-only review for unsound accepts | opus |

Soundness-critical areas (`tez_types`, `tez_verify`, `tez_solve`) get a
`soundness-reviewer` pass before merge.

## Skills

`add-syntax-feature` · `add-diagnostic` · `measure-tokens` · `run-tezbench` ·
`write-tez-example` · `soundness-check`

## Commands

`/phase` — show roadmap status and the next task · `/check` — run the full local gate

## MCP servers

| Server | Why |
|---|---|
| `context7` | Current docs for Rust crates, wasm-bindgen, Monaco, Vite, WasmGC tooling |
| `playwright` | Drive Studio in a real browser to verify UI and WasmGC execution |

Planned: `tez-mcp` (Phase 3) exposing `check`, `fmt`, `run` as tools so agents get the
JSON diagnostics loop natively. No secrets belong in `mcp.json`; use environment variables.
