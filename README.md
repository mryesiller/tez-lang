# Tez

**A fast, verified programming language that small LLMs write well and browsers run natively.**

> Status: pre-alpha — infrastructure and design only. See [`docs/ROADMAP.md`](docs/ROADMAP.md).

- **Few errors by construction** — refinement types and contracts checked by an SMT solver; what cannot be proven is rejected.
- **Built for small models** — small LLMs (reference: Gemma 4 E4B), a ≤ 1,200-token language card, a grammar for constrained decoding, structured JSON diagnostics.
- **Token-lean** — measured against TypeScript on real tokenizers.
- **WebAssembly (WasmGC)** — compiler, verifier and runtime all run in the browser.

Design: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · Plan: [`docs/ROADMAP.md`](docs/ROADMAP.md) · Contributors and agents: [`AGENTS.md`](AGENTS.md)

## Develop

```bash
make doctor          # check toolchain
make check           # format, lint, tests, wasm build, Studio build
make studio-dev      # in-browser IDE
.agents/link.sh      # (optional) expose .agents/ to Claude Code
```

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
