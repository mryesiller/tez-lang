# Tez

**A fast, verified programming language that small LLMs write well and browsers run natively.**

> Status: pre-alpha — the language works end to end (parser, type checker, SMT verifier, interpreter, formatter, WasmGC compiler, in-browser Studio). Programs may span several files; UI apps with hot-swap run in the browser, and so do the proofs (Z3 as WebAssembly). Apps can react to time and browser state, keep state in the browser and be published as one self-contained web page. The LLM tooling (repair loop, MCP server, data pipeline, TezBench) is built and measured with Gemma 4 E4B. See [`docs/ROADMAP.md`](docs/ROADMAP.md).

**Website:** <https://mryesiller.github.io/tez-lang/> · **Studio** (editor, compiler, prover and runtime in your browser): <https://mryesiller.github.io/tez-lang/studio/>

- **Few errors by construction** — refinement types and contracts checked by an SMT solver; what cannot be proven is rejected.
- **Built for small models** — small LLMs (reference: Gemma 4 E4B), a ≤ 1,200-token language card, a grammar for constrained decoding, structured JSON diagnostics.
- **Token-lean** — measured against TypeScript on real tokenizers.
- **WebAssembly (WasmGC)** — compiler, verifier and runtime all run in the browser.
- **Reads APIs** — a pure, proved `Json` reader, `Fetch.json`/`post`/`send` (allow-listed origins, no cookies), `Time` and `Html` helpers; [`examples/hn-live`](examples/hn-live) is a site that rebuilds itself every 5 minutes.
- **Live sites** — `subs` (timers, wall clock, online/visible), `fetches` and `save`/`restore` are declared as data, so apps update themselves and remember state while the program stays pure and proven. `tez site` / the Studio's *Publish* export one page under a strict Content-Security-Policy.

Design: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) · Plan: [`docs/ROADMAP.md`](docs/ROADMAP.md) · Contributors and agents: [`AGENTS.md`](AGENTS.md)

## Try it

```bash
cargo install --path crates/tez_cli      # installs the `tez` command (needs `z3` for verification)
tez check  spec/examples/13-expression-tree.tez     # types, patterns, and SMT-proved contracts
tez run    spec/examples/05-primes.tez              # verify, then run
tez build  spec/examples/13-expression-tree.tez -o out.wasm && node runtime/run.mjs out.wasm
tez fmt    program.tez --write                      # one canonical layout (a directory: every .tez file below it)
tez new    hello                                    # a first project (`--app` for a web app)
tez check  hello/main.tez --watch                   # check again whenever a file changes
```

Programs can span files: `use "other.tez"` at the top, and a file sees only the names of the files it uses
(see [`spec/examples/projects/shop`](spec/examples/projects/shop)).

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
make site-dev        # website: landing page + docs with runnable examples (docs/TOUR.md)
.agents/link.sh      # (optional) expose .agents/ to Claude Code
```

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option.
