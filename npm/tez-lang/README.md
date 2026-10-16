# tez-lang

[Tez](https://mryesiller.github.io/tez-lang/) is a small, verified language that LLMs can write and browsers can run:
every accepted program is proven free of out-of-bounds indexing, division by zero, non-termination and broken contracts,
and compiles to WebAssembly (WasmGC). This package is the compiler (as WebAssembly) and the `tez` command for Node 22+.

```sh
npm i -g tez-lang
tez check app.tez        # type-check and prove
tez run app.tez          # run `main` (compiled to WasmGC)
tez test app.tez         # run the `test` items
tez build app.tez        # write app.wasm
tez fmt app.tez --write  # canonical formatting
```

`use "other.tez"` pulls in files next to (or below) the entry file; each file sees the names of the files it `use`s directly, and everything is checked together.

Proving needs an SMT solver: the `z3` executable when it is on `PATH` (set `TEZ_Z3` to choose one), otherwise the
`z3-solver` WebAssembly build installed with this package. Without either, `tez` refuses to claim anything is verified
(`--no-verify` skips proving and says so).

Library use: `import { open } from "tez-lang"` gives `check`, `verify`, `build`, `run`, `test`, `format` over a map of files.
