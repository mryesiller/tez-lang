# Tez — Architecture Specification v0.3

> Status: draft · Date: 2026-10-14 · License: open source (MIT OR Apache-2.0)
>
> **Tez** (Turkish for *fast*) is a programming language that even 3–4B-parameter models can write accurately, whose compiled programs are proven free of defined error classes, which compiles to WebAssembly and which can be developed entirely in the browser.

## 0. Version History

**v0.6 (implementation, 2026-10)**
- Programs may span several files (`use "other.tez"`); each file sees its own names and those of the files it uses directly (`E_USE_MISSING`).
- Apps may subscribe to time and browser state (`Sub.every/clock/online/visible`), keep state in the browser (`save`/`restore`) and fetch from allow-listed origins (`fetches` → `Fetch.get`). All of it is declared as pure data; the host performs the effects.
- Verification also runs in the browser: the compiler (WebAssembly) asks for SMT queries, the page solves them with Z3 (WebAssembly) and runs the compiler again.
- A pure `Json` reader, `Time` and `Html` helpers, and general requests (`Fetch.json/post/send`) in the prelude; the host sends only allow-listed, cookie-less requests (see `examples/hn-live`).
- Programs can be published as one self-contained web page (`tez site`, Studio *Publish*) under a strict Content-Security-Policy. The compiler is also an npm package, `tez-lang`.

**v0.5 (implementation)** — the v0.4 design was implemented; deviations:
- Verification: the first version uses a **Z3 process** (SMT-LIB); the embedded `tez_solve` (L1) and Z3 WASM in the browser were postponed. `check` in the browser **does not run** verification and says so explicitly with `verified: false`.
- 61 error codes; `decreases` is now an `Int` or a data value (structural measure); recursive definitions are handed to the solver only after termination is proven.
- Code generation: a **single uniform `anyref` representation** (spec: see `crates/tez_wasm`), library in the JS runtime (`runtime/`). The interpreter stays as reference/oracle; for every program, compiled WASM ≡ interpreter is checked by differential testing.
- Syntax: no pipe (`|>`), a single way to call, method syntax (`xs.map(f)`); `List.map(xs, f)` produces an error and an automatic fix.
- Soundness: axiom consistency check, cone-of-influence slicing, random-program fuzzing (an accepted program does not violate anything while running).

**v0.4**
- **Reference model: Gemma 4 E4B** (Apache-2.0). Tokenizer, card and measurements are tuned for it (§9.0).
- One-token rule: **mandatory in the primary tokenizer (Gemma 4)**, only reported for secondary tokenizers.
- The path for Mode C became clear: `onnx-community/gemma-4-E4B-it-ONNX` + Transformers.js (`q4f16`, WebGPU).

**v0.3**
- Name: AILang/Axiom → **Tez** (`.tez`, CLI `tez`). The old name clashed with Sunholo's AILang project.
- Target model size: **3–4B**. Language card budget 2,000 → **1,200 tokens**.
- New §9: **Training set and model strategy** (compiler-verified synthetic data, fine-tuning).
- Bytecode VM removed → **direct WasmGC codegen** (Phase 2). The reference interpreter is for tests only.
- Verification in two layers: an **embedded lightweight solver (Rust, inside the WASM)** + a lazily loaded Z3 when needed.
- `Int`: arbitrary precision, **i64 fast path + promotion to bigint on overflow**.
- New §7: **Speed budgets** (compile/verify latency targets).
- New §10: Positioning (Sunholo AILang, MoonBit, etc.).

**v0.2:** Dependent types → refinement + SMT; familiar canonical syntax; TEA runtime; Worker + fuel; logos + hand-written parser.

## 1. Priorities

In design conflicts this order is the referee:

1. **Minimum errors** — a compiled program cannot produce the defined error classes (§5); the language is designed to be hard to write wrongly.
2. **LLM comprehension** — 3–4B models should write it with the language card + constrained generation, fine-tuned models with high accuracy and no card.
3. **Token economy** — express the same program in fewer tokens than TypeScript; measured with several tokenizers.
4. **Speed** — compiling, verifying and running; all in the browser (§7).
5. **WebAssembly** — a single target, WasmGC; no native/JS target.
6. **Fully in-browser development** — an editor + compiler + verifier + runtime that need no installation (+ an optional in-browser LLM).

**Out of scope (first stage):** external side effects (HTTP, storage, timers), a package manager, multi-file modules. *(Since added as pure declarations the host carries out: see v0.6.)*

## 2. Language Design Principles

1. **One canonical way.** Every concept has exactly one spelling; `tez fmt` enforces it. No variation → a small model's probability mass concentrates on a single path.
2. **Familiarity.** Every construct comes from a language that is abundant in training data (TypeScript, Rust, Kotlin, Elm). No new symbols are invented.
3. **Local reasoning.** Type signatures are mandatory on top-level functions; no shadowing, overloading or implicit conversion. Understanding a line needs no distant context — critical for the short attention span of small models.
4. **Syntax that makes mistakes impossible.** No `null`, exceptions, variable assignment, `for/while`, classes, inheritance, macros or default parameters. `&&` and `||` cannot be mixed without parentheses.
5. **LL(1), context-free grammar.** Published as GBNF → zero syntax errors with constrained generation.
6. **Single-token keywords.** Every keyword and operator must be a single token in the **primary tokenizer (Gemma 4, §9.4)**; a candidate that is not is rejected. Results for secondary tokenizers are reported but do not block.
7. **Small surface area.** The language card is ≤ 1,200 tokens. Every added feature takes a share of this budget; a feature that exceeds it is not accepted.

## 3. Syntax Sketch (v0)

```tez
// The only comment form

// --- Types ---
type Pos = Int where it > 0
type Point = { x: Int, y: Int }
type Shape = Circle(r: Pos) | Rect(w: Pos, h: Pos)

// --- A function body is always `= expression` ---
fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Rect(w, h) => w * h
}

fn div(a: Int, b: Int where it != 0) -> Int = a / b

// --- Contracts ---
fn avg(xs: List[Int]) -> Int
  requires len(xs) > 0
  = sum(xs) / len(xs)

fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  ensures lo <= result && result <= hi
  = if x < lo { lo } else if x > hi { hi } else { x }

// --- Recursion: a termination proof is mandatory ---
fn fact(n: Int where it >= 0) -> Int
  decreases n
  = if n == 0 { 1 } else { n * fact(n - 1) }

// --- Block, let, pipe, lambda ---
fn score(xs: List[Int]) -> Int = {
  let evens = xs |> List.filter(x => x % 2 == 0)
  List.fold(evens, 0, (acc, x) => acc + x) * 2
}

// --- Immutable record update ---
fn moveRight(p: Point) -> Point = { p with x: p.x + 1 }
```

| Decision | Choice | Why |
|---|---|---|
| Block | `{ }` | Indentation sensitivity is fragile in small models and in copy/paste; a missing brace is detected locally |
| Line end | No semicolons | Token economy |
| Generics | `List[Int]` | `<>` is ambiguous with comparison |
| Lambda | `x => e`, `(a, b) => e` | Most common in TypeScript |
| Refinement | `T where it > 0` | Readable English; `it` is from Kotlin |
| Contract | `requires` / `ensures` / `decreases` | The Dafny standard |
| Body | Always `= expression` | One canonical way |
| Match arm | One arm per line, no commas | Fewer tokens and errors |
| Naming | values `camelCase`, types `PascalCase` | One rule; the parser tells them apart by the capital letter |
| Access | `List.map` | No overloading or name clashes |

> This table is finalised by the tokenizer and small-model measurements of Phase 0 (§9). If a measurement shows a decision to be bad, the decision changes.

## 4. Type System

- **Base types:** `Int` (arbitrary precision), `Float` (f64), `Bool`, `Str`, `Unit`.
- **Compound:** records, sum types (ADTs), `List[T]`, `Map[K, V]`, `Option[T]`, `Result[T, E]`.
- **Parametric polymorphism** (generics); **no** traits/typeclasses (to be considered in v1 — the complexity cost is high for small models).
- **Bidirectional checking:** top-level signatures are mandatory; local HM inference inside functions.
- **Refinement types:** `T where P(it)`, where P is a predicate in the decidable fragment (§5.1).
- **`Int` representation:** `i31ref`/i64 fast path in WasmGC; promoted to a bigint object on overflow. Overflow is never an error and carries no proof burden.

## 5. Guarantee Model

> **If compilation succeeds, the program cannot produce any of the following error classes at run time and it honours all written contracts. Every case that cannot be proven is rejected (sound).**

No system can prove that a program does *what was intended*; logic errors are caught only to the extent they are expressed with `ensures`. This is why the training set teaches writing a contract for every function (§9).

| Error class | Mechanism | Phase |
|---|---|---|
| Null / undefined | No `null`; `Option[T]` | 1 |
| Type mismatch | Bidirectional checking | 1 |
| Missing match arm | Exhaustiveness checking | 1 |
| Uncaught exception | No exceptions; `Result[T, E]` | 1 |
| Side effect / shared state | Pure language, immutable data | 1 |
| Undefined name | Name resolution | 1 |
| Integer overflow | Arbitrary-precision `Int` | 1 |
| Division by zero | `b != 0` VC | 4 |
| Out-of-bounds access | `0 <= i < len(xs)` VC | 4 |
| Contract violation | `requires`/`ensures`/`where` VC | 4 |
| Non-termination | No loops; `decreases` VC; only bounded `List.fold/map/range` | 4 |
| Float NaN/∞ | `b != 0.0` for `/`, `x >= 0.0` for `sqrt` VC | 4 |
| Browser freeze | Termination proof + Worker + fuel (defence in depth) | 2 |
| Sandbox escape | WASM capability model: only the imports the runtime provides | 2 |
| Memory exhaustion | Runtime ceiling → controlled `E_RESOURCE` (the only class that cannot be proven statically) | 2 |

### 5.1 Two-layer verification (for speed)

| Layer | What | Where | Scope |
|---|---|---|---|
| **L1 — `tez_solve`** | A lightweight decision procedure written in Rust: linear integer arithmetic (simplex + branch-and-bound / Fourier–Motzkin), equality + uninterpreted functions (congruence closure), `len` measures | In the same `.wasm` as the compiler | In practice the vast majority of VCs (division by zero, bounds, simple contracts); target < 1 ms/VC |
| **L2 — Z3** | `z3-solver` (the official WASM build) | A separate Worker; loaded lazily **only when L1 says `unknown`** | Non-linear arithmetic, complex quantifiers |

- If L2 cannot be loaded or times out → `E_UNPROVEN` + a suggestion. Soundness is never sacrificed for speed.
- Z3 needs `SharedArrayBuffer` in the browser → the page is served with COOP/COEP headers.
- Every VC is cached by the hash of its normalised form; unchanged functions are not re-proven.

## 6. Compiler Architecture

```
source (.tez)
  ▼ tez_syntax   logos lexer → recursive-descent parser (with error recovery) → AST + span
  ▼ tez_resolve  name resolution
  ▼ tez_types    bidirectional checking + local HM + exhaustiveness + refinement propagation
  ▼ tez_verify   VC generation → L1 tez_solve → (if needed) L2 Z3
  ▼ tez_ir       Core IR (ANF), refinements erased
  ▼ tez_wasm     WasmGC codegen (wasm-encoder)  ──►  .wasm module
  ╰ tez_interp   reference interpreter (tests only: differential testing against codegen)
Every stage ─► tez_diag  one uniform Diagnostic → JSON
```

**Incrementality:** The unit of compilation is the function. The pair (signature hash, body hash) of each function caches the type-checking, VC and codegen results. A keystroke in the editor re-processes only the changed function, and its callers if its signature changed.

**Why WasmGC and not a VM:** WasmGC is supported in all major browsers; the browser provides the garbage collector → small output, no GC of our own to write, near-native speed. Since `WebAssembly.compile` of small modules takes milliseconds, a separate VM is unnecessary.

### Repository layout

```
tez/
├── .agents/         # shared agents, skills, commands, MCP (single source of truth)
├── docs/            # ARCHITECTURE.md, ROADMAP.md
├── crates/
│   ├── tez_syntax/  tez_resolve/  tez_types/  tez_verify/  tez_solve/
│   ├── tez_ir/      tez_wasm/     tez_interp/  tez_diag/   tez_fmt/
│   ├── tez_cli/     # `tez check | run | build | fmt | grammar`
│   └── tez_web/     # wasm-bindgen API
├── runtime/         # TS: TEA loop, Virtual DOM diff, Worker host, hot-swap
├── studio/          # React + Monaco: the in-browser IDE
├── spec/
│   ├── grammar.ebnf      # normative
│   ├── grammar.gbnf      # constrained generation
│   └── card.md           # language card, ≤ 1,200 tokens
├── data/            # training-set pipeline (§9)
└── bench/           # TezBench
```

## 7. Speed Budgets

Measured in the browser on a mid-range laptop. Tracked as regression tests in CI.

| Operation | Target |
|---|---|
| Compiler `.wasm` size (gzip, without Z3) | < 1.5 MB |
| Studio first open (without Z3) | < 1.5 s |
| Keystroke → diagnostics (1,000 lines, incremental, L1) | < 50 ms |
| Full compile + verify (1,000 lines, L1, cold) | < 300 ms |
| Save → hot-swap → new screen | < 100 ms |
| L2 (Z3) first load | in the background; does not block the editor |

## 8. Runtime: Elm Architecture + Sandbox

```tez
type Model = { count: Int where it >= 0 }
type Msg = Inc | Dec

fn init() -> Model = { count: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Inc => { m with count: m.count + 1 }
  Dec => if m.count > 0 { { m with count: m.count - 1 } } else { m }
}

fn view(m: Model) -> Html[Msg] =
  row([ button(Dec, "-"), text(m.count), button(Inc, "+") ])
```

If `else { m }` is deleted, the verifier rejects the `count >= 0` violation with the counterexample `m.count = 0, msg = Dec`.

- **Execution:** The module runs in a Web Worker; every `update`/`view` call is bounded by fuel (a counter codegen inserts at loop/recursion points) and a memory ceiling.
- **DOM:** The Worker sends the Virtual DOM tree to the main thread; the main thread diffs and applies it; events come back as `Msg`.
- **Hot-swap:** `Model` is serialised with a canonical value encoding → the new module is instantiated → the state is restored. If the `Model` type changed, a structural migration (fields with the same name and type are carried over, new ones get defaults); if that is impossible, `init()` and a notice to the user.
- **Side effects (Phase 7+):** TEA `Cmd`/`Sub`.

## 9. LLM Strategy: Training Set, Model, Measurement

### 9.0 Reference model: Gemma 4 E4B

| Property | Value (source: model card) |
|---|---|
| License | Apache-2.0 → we can publish the fine-tuned weights and the dataset openly |
| Size | 4.5B effective parameters, 8B total with embeddings (Per-Layer Embeddings) |
| Vocabulary / context | 262K tokens / 128K |
| Architecture | 42 layers, a mix of 512-token sliding-window and global attention; has image and audio encoders |
| Code performance (instruction-tuned) | LiveCodeBench v6: 52.0% · Codeforces ELO: 940 |
| Recommended sampling | temperature 1.0 · top_p 0.95 · top_k 64 |
| Thinking mode | switched on/off with `enable_thinking` |
| Browser | `onnx-community/gemma-4-E4B-it-ONNX`, Transformers.js, `q4f16`, WebGPU |

Consequences and design implications:
- The **262K vocabulary** raises the chance that common words are single tokens; every keyword is still measured (§2, principle 6).
- The **card budget** (1,200 tokens) comes not from the context limit but from attention quality in a small model and from the prefill latency in the browser; the 128K context does not change that.
- **Benchmark protocol:** every measurement is reported with both *greedy* and the recommended sampling, with thinking mode off **and** on. The Mode C default is thinking off, for latency.
- **Fine-tuning risk (Phase 5):** the model ships with a multimodal wrapper (`Gemma4ForConditionalGeneration`) and has PLE tables; LoRA will be applied to the text decoder only. Tooling support (e.g. Unsloth releases) appears to exist but **must be verified with a small trial fine-tune in Phase 0**.
- **Mode C risk (measured, 2026-10-15):** `onnx-community/gemma-4-E4B-it-ONNX` text-only `q4f16` = **4.9 GB** (decoder 2.9 + `embed_tokens` 2.0; vision +0.1, audio +0.17); q4 5.6 GB; fp16 16.5 GB (`bench/onnx-sizes.mjs`). This is heavy for a browser: Mode C is offered as an "optional download" (download once, keep in Cache Storage); a smaller fine-tuned model (e.g. ≤ 1–2 GB) is the target for Phase 5/6.
- Other 3–4B models are **supported but not targeted**: the card and syntax are tuned for Gemma 4; the others are regression indicators.

### 9.1 Usage modes

| Mode | Model | Mechanism |
|---|---|---|
| **A — Zero fine-tuning** | Gemma 4 E4B (reference); other 3–4B+ models | `card.md` + a few examples + GBNF constrained generation + error loop |
| **B — Fine-tuned** | Tez-Coder = a Gemma 4 E4B fine-tune (open weights, Apache-2.0) | LoRA/full fine-tuning; no card needed |
| **C — Fully in the browser** | Tez-Coder, `q4f16` ONNX | WebGPU (Transformers.js) + GBNF; no server, no network |

Mode C is the far end of the "fully in-browser development" goal: editor, compiler, verifier, runtime **and model** on the user's device.

### 9.2 Training-set pipeline

Because the compiler is a **perfect verifier**, data can be produced largely synthetically and verified automatically:

1. **Seed tasks:** Openly licensed problem sets (MBPP, HumanEval style) + TEA UI tasks; a natural-language description + tests.
2. **Generation:** A large model, with `card.md`, produces N candidate Tez programs for every task.
3. **Rejection sampling:** Only those that pass `tez check` (types + verification) and the tests are kept.
4. **Canonicalisation:** All samples go through `tez fmt` → a single style in the dataset.
5. **Repair traces:** Failed candidates are not thrown away; they are stored as `(faulty code, JSON error) → repaired code` pairs. The model learns not only to write but **the loop itself**.
6. **Contract enrichment:** Variants with `ensures` added to every function; the model acquires the habit of writing contracts.
7. **Mutation data:** Controlled faults are injected into correct programs (bounds, division by zero, missing arm) → error/repair pairs.
8. **Held-out test set:** TezBench tasks are never used in training (contamination check).

### 9.3 TezBench

- 100+ tasks: pure functions, data transformations, TEA apps; each with tests and contracts.
- Comparison: TypeScript versions of the same tasks, with the same models.
- Model classes: **Gemma 4 E4B (main target)**, a secondary small model (regression indicator), a frontier model.

| Metric | Definition | Target (calibrated in Phase 0) |
|---|---|---|
| Syntax validity | Output that parses on the first try | 100% with GBNF; without GBNF, Mode A 3–4B ≥ 90% |
| pass@1 (Mode A, 3–4B) | Passes all checks on the first try | Equal to or better than TypeScript |
| pass@1 (Mode B, 3–4B) | Fine-tuned | Clearly above Mode A |
| Loop convergence | Passes within ≤ 3 turns | ≥ 90% |
| Token ratio | Tez / TS token count | ≤ 0.8 (on the Gemma 4 tokenizer; reported for the secondary ones) |

### 9.4 Tokenizer set

**Primary:** Gemma 4 (`google/gemma-4-E4B`, `tokenizer.json`, 262K vocabulary). Design decisions are made against it; the one-token rule and the token-ratio target are **binding** here.

**Secondary:** one tokenizer each from two different families (chosen in Phase 0). Reported only; they exist to show that Tez is not over-fitted to a single tokenizer. If there is a marked degradation on a secondary one, the decision is reviewed but not rejected automatically.

## 10. Structured Error Schema

```json
{
  "version": 1,
  "status": "error",
  "diagnostics": [{
    "code": "E_REFINEMENT",
    "phase": "verify",
    "message": "The divisor may be zero",
    "span": { "start": { "line": 7, "col": 14 }, "end": { "line": 7, "col": 19 } },
    "expected": "len(xs) != 0",
    "counterexample": { "xs": "[]" },
    "fixes": [{
      "title": "Add a precondition",
      "edits": [{ "span": { "start": { "line": 6, "col": 33 }, "end": { "line": 6, "col": 33 } },
                  "replace": "\n  requires len(xs) > 0" }]
    }]
  }]
}
```

- An edit replaces the `span` range with the `replace` text; an empty range = an insertion.
- Stable codes: `E_PARSE_*`, `E_TYPE_*`, `E_MATCH_*`, `E_REFINEMENT`, `E_UNPROVEN`, `E_TERMINATION`, `E_RESOURCE`.
- A parser with error recovery → all errors in a single compile.
- Messages are short and templated — they add few tokens to a small model's context; the training set uses the same templates.

## 11. Positioning

| Project | Relation | How Tez differs |
|---|---|---|
| **AILang (Sunholo)** | A pure functional language for AI-written code; explicit effects | Tez: SMT-proven error classes, a 3–4B target with an open training set/model, WasmGC + fully in-browser development |
| **MoonBit** | AI-friendly, WASM-first | Tez: formal verification, a minimal surface focused on small models |
| **Elm** | Pure, TEA, no run-time errors | Tez: refinements/contracts, an LLM loop, WASM |
| **Dafny / F\*** | SMT-based verification | Tez: a web-app runtime, LLM-first syntax, runs in the browser |
| **v0 / Bolt / Lovable** | Prompt → web-app products | Tez: a compiler that gives guarantees; an open stack that can run entirely locally |

**In one sentence:** *Tez is a language in which even a small model can write and instantly run programs in the browser, and which, once compiled, is proven unable to contain certain errors.*

## 12. Roadmap

| Phase | Duration | Output | Exit criterion |
|---|---|---|---|
| **0. Language design + measurement** | 2 weeks | `grammar.ebnf/.gbnf`, `card.md`, 30 example programs, tokenizer measurement tool, TezBench skeleton (20 tasks) | Mode A: ≥ 80% syntax validity on a 3–4B model; token ratio ≤ 0.9 |
| **1. Core** | 3 weeks | lexer, parser, name resolution, type checking, exhaustiveness, `tez_interp`, JSON errors, `tez fmt`, CLI | The §5 Phase-1 classes proven by tests |
| **2. WasmGC + browser** | 3 weeks | `tez_wasm`, `tez_web`, Studio (Monaco), TEA runtime, Worker, fuel, hot-swap | The counter example is live in the browser; the §7 budgets (except L1) hold |
| **3. LLM loop + data pipeline v1** | 2 weeks | LLM integration in the Studio, automatic repair, the first version of the §9.2 pipeline | **First end-to-end demo**; first TezBench measurement |
| **4. Verification** | 4 weeks | refinements, VC generation, `tez_solve` (L1), Z3 (L2), counterexamples, `decreases`, VC cache | All classes in §5 active; L1 < 1 ms/VC |
| **5. Tez-Coder v0** | 3 weeks | Training set v1 (verified + repair traces), a 3–4B fine-tune, open-weights release | Mode B pass@1 clearly above Mode A |
| **6. In-browser model** | 2 weeks | Mode C: WebGPU + GBNF in the Studio | An end-to-end demo without internet |
| **7. Effects** | later | `Cmd`/`Sub`, HTTP, storage | — |

## 13. Open Questions

1. ~~Name check~~ **Decision: the brand is Tez, the package/repository is `tez-lang`** (published, 2026-10-14). Open: the domain.
2. ~~Reference model~~ **Decision: Gemma 4 E4B** (§9.0). Open: two secondary tokenizer families, the Mode C download size, the LoRA trial.
3. **A large model for data generation:** which model/API will be used in §9.2 step 2? (Check the output licence terms.)
4. **Studio hosting:** static hosting (with COOP/COEP header support) — which platform?
