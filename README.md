# Tez

**A fast, verified programming language that small LLMs write well and browsers run natively.**

> Status: pre-alpha — the language works end to end (parser, type checker, SMT verifier, interpreter, formatter, WasmGC compiler, in-browser Studio). Not yet: UI runtime, LLM tooling. See [`docs/ROADMAP.md`](docs/ROADMAP.md).

- **Few errors by construction** — refinement types and contracts checked by an SMT solver; what cannot be proven is rejected.
- **Built for small models** — small LLMs (reference: Gemma 4 E4B), a ≤ 1,200-token language card, a grammar for constrained decoding, structured JSON diagnostics.
- **Token-lean** — measured against TypeScript on real tokenizers.
- **WebAssembly (WasmGC)** — compiler, verifier and runtime all run in the browser.

Design: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · Plan: [`docs/ROADMAP.md`](docs/ROADMAP.md) · Contributors and agents: [`AGENTS.md`](AGENTS.md)

## Try it

```bash
cargo install --path crates/tez_cli      # installs the `tez` command (needs `z3` for verification)
tez check  spec/examples/13-expression-tree.tez     # types, patterns, and SMT-proved contracts
tez run    spec/examples/05-primes.tez              # verify, then run
tez build  spec/examples/13-expression-tree.tez -o out.wasm && node runtime/run.mjs out.wasm
tez fmt    program.tez --write                      # one canonical layout
```

```tez
fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  ensures lo <= result && result <= hi
  = if x < lo { lo } else if x > hi { hi } else { x }
```

A program is accepted only if the compiler *proves* it free of the error classes in
[`spec/language.md`](spec/language.md#7-guarantees-and-verification); every violation is reported with a
stable code, a counterexample and, where possible, a one-click fix ([`spec/errors.md`](spec/errors.md)).

## Develop

```bash
make doctor          # check toolchain
make check           # format, lint, tests, wasm build, Studio build
make studio-dev      # in-browser IDE
.agents/link.sh      # (optional) expose .agents/ to Claude Code
```

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
