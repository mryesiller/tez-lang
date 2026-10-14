# Tez — instructions for AI agents

Tez is an open-source language that 3–4B-parameter LLMs can write accurately, whose compiled programs are proven free of defined error classes, compiled to WasmGC and developed entirely in the browser.

Read first: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) (design) and [`docs/ROADMAP.md`](docs/ROADMAP.md) (what to do next). Agents, skills, commands and MCP config live in [`.agents/`](.agents/README.md); run `.agents/link.sh` once if your tool expects `.claude/` or `.mcp.json`.

## Priorities (in order)
1. Minimum errors — the compiler is **sound**: anything it cannot prove is rejected.
2. LLM comprehension (3–4B models) 3. token economy 4. speed 5. WasmGC 6. fully in-browser.

## Non-negotiables
- **Never make the compiler accept more programs to get a test or demo passing.** Fix the cause. Unimplemented paths fail loudly; a stub that says "ok" is a bug.
- No `unwrap`/`expect`/`panic`/`todo!` in non-test code (workspace lints). The compiler must not panic on any input.
- Language/syntax claims need measurements (`measure-tokens`, `run-tezbench`), not intuition.
- The language card stays ≤ 1,200 tokens; every keyword is one token in all target tokenizers.
- TezBench tasks never enter training data.
- Diagnostic codes are stable; JSON schema changes bump `SCHEMA_VERSION`.

## Workflow
- Pick the next task from `docs/ROADMAP.md` (or run `/phase`); delegate to the owning agent in `.agents/agents/`.
- Gate before reporting done: `make check`.
- Changes to `tez_types`, `tez_verify`, `tez_solve` get a `soundness-reviewer` pass.
- Update `docs/ROADMAP.md` checkboxes and `docs/ARCHITECTURE.md` when decisions change.

## Layout
```
crates/   Rust workspace (tez_diag, tez_syntax, tez_cli; more per phase)
studio/   React + Monaco in-browser IDE (Vite, pnpm)
runtime/  TypeScript runtime (Phase 2)
spec/     grammar.ebnf (normative), grammar.gbnf (generated), card.md, examples/
bench/    tokenizer measurements and TezBench
data/     training-data pipeline
docs/     architecture and roadmap
.agents/  shared agents, skills, commands, MCP config
```

## Tooling
Rust via rustup (version pinned in `rust-toolchain.toml`, wasm32 target included); Homebrew's plain `rust` lacks wasm32, so use `make` (it puts rustup first). Node 22, pnpm, `wasm-tools`, `binaryen`, Z3 are expected; `make doctor` checks them.
