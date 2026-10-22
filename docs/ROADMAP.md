# Tez — Roadmap

> Source: [ARCHITECTURE.md](ARCHITECTURE.md) §12 · Updated: 2026-10-14
> This file is the **live status** of the work. When an item is done, `[ ]` → `[x]`. The next phase does not start until the phase's exit criterion is met.

Status: `⬜ not started` · `🟦 in progress` · `✅ done` · item markers: `🔒` waiting on an external resource/approval (model, GPU, release) · `⏸` deliberately postponed (with the reason) · `[~]` partly done

| Phase | Name | Duration | Status |
|---|---|---|---|
| A | Infrastructure and agent environment | 1 day | 🟦 |
| 0 | Language design + measurement | 2 weeks | ✅ (Phase 0 exit criterion "≥ 80% syntax validity": 85% with the card, 90% with feedback ✓) |
| 1 | Core compiler | 3 weeks | ✅ |
| 2 | WasmGC + browser | 3 weeks | ✅ (fast path ⏸) |
| 3 | LLM loop + data pipeline v1 | 2 weeks | 🟦 (tools ✅, model measurement 🔒) |
| 4 | Verification (refinement + SMT) | 4 weeks | ✅ (filter/map axiom ⏸) |
| 5 | Tez-Coder v0 (fine-tuning) | 3 weeks | 🟦 (data ✅, training 🔒) |
| 6 | In-browser model | 2 weeks | ⬜ |
| 7 | Effects (`Cmd`/`Sub`) | later | ⬜ |

---

## Phase A — Infrastructure and agent environment 🟦

- [x] Repository layout, git, licences (MIT OR Apache-2.0)
- [x] `.agents/`: agents, skills, commands, MCP configuration
- [x] Rust workspace + `tez_diag`, `tez_syntax`, `tez_cli` skeletons
- [x] `rust-toolchain.toml`, `rustfmt`, `clippy` configuration
- [x] `Makefile` (check, test, wasm, studio)
- [x] Studio skeleton (Vite + React + Monaco, COOP/COEP headers)
- [x] CI workflow (`.github/workflows/ci.yml`)
- [x] Toolchain: `rustup` (1.99.0 pinned) + `wasm32-unknown-unknown`, `wasm-tools`, `binaryen`, Z3 — `make doctor` green
- [x] First CI runs green; z3 installation added
- [x] Remote repository set: `origin` = github.com/mryesiller/tez-lang (the user creates it)
- [x] First commit and first push (with the user's approval)
- [x] **Name check (2026-10-14):** `tez` is taken on crates.io and npm → package name **`tez-lang`** (free on both; the command stays `tez`). Internal crates `tez_*` (all free on crates.io). GitHub repository: `mryesiller/tez-lang` (same name as crates.io and npm)
- [x] **Name reservation (2026-10-14):** `tez-lang` 0.0.1 is published on both crates.io and npm. The internal `tez_*` crates are published when they are used in Phase 1

**Exit criterion:** `make check` green (✅ verified locally), CI green, name check done.

## Phase 0 — Language design + measurement (partly)

The language design is complete and implemented; the LLM measurements were postponed (user decision: language first).

- [x] Reference model: **Gemma 4 E4B** (Apache-2.0) → [ARCHITECTURE §9.0](ARCHITECTURE.md)
- [x] `spec/grammar.ebnf` (normative), `spec/language.md` (semantics), `spec/errors.md` (64 error codes, kept in sync by a test)
- [x] Example programs: 14 accepted + 10 rejected (`spec/examples/`, enforced by a test)
- [x] 14 interactive app examples (15, 16, 41–54): counter, echo, countdown, show/hide panel + theme, todo list, temperature converter, tabs, tic-tac-toe, shopping cart, quiz, traffic light (class change), colour mixer (`style` attribute), password strength, live filter, word counter, a full filtered todo app (54) — all verified and tested
- [x] Completed to 30 accepted examples (+10 rejected); each has a TypeScript counterpart in `bench/ts/` (except the app examples)
- [x] `bench/tokens`: the Gemma 4 tokenizer (downloaded, version pinned) + a measurement tool; **36/38 keywords/operators are one token**, Tez/TS token ratio **0.88** (28 paired programs; 0.84 without the proof lines; it was 0.82 on the first 14 programs and rose as examples were added — in short programs the contract lines weigh heavily)
- [x] `spec/card.md`: **930 tokens** (budget 1,200); its examples are verified, its method list is kept in sync with the prelude by a test
- [x] `spec/grammar.gbnf`: generated from the EBNF by `spec/tools/ebnf2gbnf.py`; `--check` verifies that it is up to date and that the 30 examples are derivable (a superset: whitespace/newlines are free, keywords are not excluded from identifiers)
- [x] TezBench skeleton: `bench/tezbench/` — 20 tasks, Tez + TypeScript references, a four-level judge (parses, type-checks, verifies, passes the cases), `--self-check`, `--prompts`
- [x] `q4f16` ONNX sizes measured (HF metadata only, no weights downloaded; `bench/onnx-sizes.mjs`): **text-only q4f16 = 4.9 GB** (decoder 2.9 + embeddings 2.0), q4 5.6 GB, fp16 16.5 GB. Conclusion: Mode C must be an "optional download" (the ARCHITECTURE §9 risk is confirmed); a smaller fine-tuned model is a meaningful goal
- [x] Small trial LoRA (2026-10-15, Mac M4 Pro 25 GB, `mlx-lm`, local 4-bit Gemma 4 E4B; no new weights downloaded): `data/lora/` — 227 examples (26 example programs + 77 verified programs rejection-sampled from Gemma + 124 repair traces; TezBench overlap 0), rank 16, 16 layers, 600 steps, peak memory ≈ 6 GB (~10 min). Validation loss 2.94 → 0.16. **TezBench (same model, greedy, 20 tasks):**

  | Mode | before | after LoRA (passes) | (verifies) |
  |---|---|---|---|
  | no card | 0% | **70%** | 70% |
  | card | 65% | 50% | 60% |
  | card + 3 feedback turns | 55% | 60% | 75% |

  Reading: fine-tuning removed the need for the card (0% → 70%, syntax validity 100% with the card); the card modes stayed the same or dropped slightly (the fine-tuning data had no card). Caveats: 20 tasks, a single sample (±10%); the training prompts are independent of TezBench but cover the same area, short functions; the weights are not published (`data/lora/adapters` is outside git)
- [x] First measurement (2026-10-15, LM Studio, `google/gemma-4-e4b`, greedy, 20 tasks, a single attempt; `bench/tezbench/generate.mjs`, results in `bench/results/tezbench-gemma4-e4b-*`):

  | Mode | valid syntax | verifies + passes the tests |
  |---|---|---|
  | Tez, no card | 0% | 0% |
  | Tez, card (first version) | 75% | 30% |
  | Tez, card + 3 turns of compiler feedback | 90% | 55% |
  | Tez, card (method-first sentence added) | 85% | 65% |
  | TypeScript (comparison) | — | 100% |

  A second ~4B model (`microsoft/phi-4-mini-reasoning`, card, first version): valid syntax 75%, verifies+passes **5%** — the result depends heavily on the model; Gemma 4 E4B's 30% is not an upper bound specific to Tez, it is specific to Gemma.

  Reading: without a card the model cannot write Tez at all; card + feedback lifts it to 55–65% without fine-tuning, still far behind TypeScript — fine-tuning is really needed (Phase 5). The dominant error: writing hand-made recursion over lists instead of methods, and forgetting `decreases`. One general sentence in the card (`Before writing recursion over a list, use these methods`) took it 30% → 65%. **Caveat:** that sentence was added after looking at the TezBench failures (a slight leak into the card design); new measurements need a separate held-out task set next to TezBench. A single sample, 20 tasks: ±10% uncertainty

## Phase 1 — Core compiler ✅

- [x] `tez_syntax`: lexer, recursive-descent parser with error recovery, naming rules + automatic fixes (46 tests)
- [x] `tez_types`: name resolution, bidirectional inference + unification, method resolution, exhaustiveness/reachability (Maranget), termination call graph, typed HIR (32 tests)
- [x] `tez_interp`: the reference interpreter — unbounded Int, closures, resource limits, optional contract checking (18 tests)
- [x] `tez_diag`: stable error codes (64 today) + JSON schema v1
- [x] `tez_fmt`: the canonical formatter, preserves or rejects comments (14 tests, prelude idempotent)
- [x] `tez_cli`: `check | run | test | fmt | grammar` (`--json`, `--no-verify`, `--contracts`)
- [x] Fuzz: `crates/tez_driver/tests/robust.rs` — seeded mutation fuzzing (`TEZ_FUZZ_N` per example, ~100 thousand mutations with 4000): the front end does not crash, `fmt(fmt(x)) == fmt(x)`, formatted output re-parses. It found a real bug on the first run (the `(p,)` tuple pattern) and it was fixed
- [x] Differential-testing infrastructure (WASM output ≡ interpreter) → in Phase 2 as `tez_wasm/tests/differential.rs`

## Phase 2 — WasmGC + browser ✅

- [x] `tez_wasm`: WasmGC code generation (`wasm-encoder`) — a single uniform `anyref` representation (small Int `i31ref`, large Int JS `BigInt`, ADT/tuple GC structs, closures `call_ref`)
- [x] `runtime/tez-runtime.mjs`: the built-in library + big numbers + structural equality + value printer (Node and browser)
- [x] Differential test: compiled WASM ≡ interpreter (7 groups, 120+ fragments, 14 examples, run-time errors; with `wasm-tools validate`)
- [x] `tez build`; `tez_web` (`check/format/run/test/build`), browser compiler **257 KB gzip**
- [x] Studio: the compiler in a Worker, live Monaco diagnostics, one-click fixes, format/run (compiled WASM)/test, example picker
- [x] Speed: `fib(30)` ≈ 0.03 s (WASM) vs 0.65 s (native interpreter); recursion 10,000+ deep
- [x] `runtime/tez-app.mjs`: the TEA loop (the `Html[Msg]` type is in the prelude, vnode diff, click/input events); an "App" button in the Studio; examples 15-counter-app, 16-echo-app
- [x] Hot-swap: an event log (event sourcing) — when the code changes, the new program starts from `init()` and the user's events are replayed; works in the Studio and on the site
- [x] Hot-swap: replaying an event log instead of serialising state (no structural migration needed: the program is pure, the model is recomputed every time)
- [x] `site/`: landing page + documentation (Tour, reference, error catalog, examples, architecture, roadmap); runnable examples are compiled in the browser; a GitHub Pages workflow (`.github/workflows/pages.yml`)
- [x] Run limits: the compiler in a Worker; if running/testing passes 10 s the worker is killed and restarted (`E_RESOURCE`), list/power/big-number ceilings and memory errors map to `E_RESOURCE`. A wall clock was chosen instead of a fuel counter (adding a counter to compiled code slows every call). Limit: the `update`/`view` calls of TEA apps run on the main thread
- [x] The Studio bundle was shrunk: the Monaco core + only the needed contributions (`studio/src/monaco-lite.js`); the first-loaded JS 4.3 MB → 3.45 MB (gzip 1.08 → 0.89 MB), `dist` ≈ 13 MB → 5 MB (the ts/css/html workers are gone)
- [x] Verification in the browser: Z3 WebAssembly + a **replay backend** — the verifier does not wait for the solver; for an answer it does not have, it receives `unknown` (it can only reject) and records the script that is needed; the host solves the scripts asynchronously; the verifier is re-run with the answers, until nothing is missing. `tez_verify/tests/replay.rs`: on 30 programs the result is identical to the process backend; a **Verify** button in the Studio (36-tree-depth: 211 queries, 15 rounds, 1.2 s). Z3 is 33 MB and is downloaded only when Verify is pressed; it needs cross-origin isolation (a dev server + `coi-sw.js` for Pages)
- [x] Incremental compilation was not needed (measured): a 1,561-line program type-checks locally in 0.08 s; small examples are too fast to measure. Verification is the expensive part and the VC cache (script hash) already covers it. A per-function type cache brings a correctness risk with no gain — to be reopened if a program in the browser passes 5,000 lines and a measurement exceeds the budget
- [x] Code generation: tail calls (`return_call`/`return_call_ref`; in `if`/`match` arms as well; recursion 5 million deep runs in constant stack; requirement: Chrome 112+/Firefox 121+/Safari 18.2+/Node 20+), a nullary ADT singleton (no allocation per use)
- [ ] ⏸ Code generation: a local Float/Int fast path (unboxed local variables). Postponed — measurement: `fib(35)` (9,227,465) compiled Tez ≈ 0.09 s (excluding Node start-up), the same function in plain JavaScript 0.05 s: the boxed `i31` path is already within ~2× of plain JS (the integer helpers take the fast path with an `i31` test); type-aware local types are a separate refactor. To be opened if a benchmark shows a bottleneck

**Exit criterion:** a TEA application (counter) live in the browser; the ARCHITECTURE §7 budgets.

## Phase 3 — LLM loop + data pipeline v1 🟦 (tools ✅, model measurement 🔒)

- [x] LLM integration in the Studio: an "Ask a model" panel — any OpenAI-compatible server (`tools/agent/providers.mjs`), card + error feedback, verified end to end in the browser (with a fake server)
- [x] The automatic repair loop: `tools/agent/loop.mjs` (shared by Node and the browser), the `solve.mjs` CLI, tested with the real compiler
- [x] The `data/` pipeline v1: `sample.mjs` (task → N candidates → compiler rejection → canonicalisation → de-duplication → contamination filter), `build-v1.mjs`
- [x] Repair-trace generation: with model-free mutation (`data/mutate.mjs`, 11 operators, 17 error codes) and from model attempts
- [ ] Full TezBench measurement: 4B (Gemma 4 E4B) ✅ above; the 7–8B and frontier-model columns 🔒 (another model/API key is needed in LM Studio: `node bench/tezbench/generate.mjs --base-url … --model …`)
- [x] `tez-mcp` (`tools/tez-mcp/`): `tez_check`, `tez_fmt`, `tez_run`, `tez_test`; a dependency-free stdio MCP server, tested

**Exit criterion:** the first end-to-end demo; convergence in ≤ 3 turns ≥ 90% (a mid-size model).

## Phase 4 — Verification ✅

- [x] Refinement types (`T where P`), contracts, structural + numeric `decreases`
- [x] VC generation (symbolic execution): division by zero, Float domain, index, precondition, postcondition, refinement, assert, termination
- [x] Solved with Z3 (an SMT-LIB process); every violation has its own code + a **counterexample**; what cannot be proven is rejected (`E_UNPROVEN`)
- [x] Inconsistent-axiom guard; cone-of-influence slicing; recursive definitions only after termination is proven
- [x] Lists (length/element/append/take/drop/concat/rev/range), strings, floats, ADTs, tuples
- [x] Lambda parameters are known as list elements (`map/filter/all/any/count/flatMap/sortBy/fold`)
- [x] Prelude bodies satisfy their own contracts (tested)
- [x] 27 verifier tests (acceptance + a rejection for every code + contradiction + soundness traps)
- [ ] ⏸ Contracts about the elements of a `filter/map` result (`all(pred)`, element equality) — axiomatising the lambda body as a closed term. Postponed: the real goal (quicksort length preservation) needs induction for the partition lemma; an element axiom alone does not solve it, and axiomatising lambda captures (env) carries a soundness risk. The working solution: the user writes a contract on their own function (language.md §9)
- [x] `tez_solve` (L1) was not needed: verification in the browser was solved with Z3 WASM (above). Our own solver would make sense only if we want to shrink the 33 MB; postponed because of the soundness risk
- [x] The VC cache: the SHA-256 of the script sent to the solver (including the solver version); only `unsat` is stored, so a wrong entry can never turn a rejection into an acceptance; cold 0.90 s → warm 0.32 s; `TEZ_CACHE_DIR`, `TEZ_NO_CACHE`; a `cache_dir` option
- [x] Soundness fuzz: `tez_verify/tests/soundness.rs` — 300 programs, 75 verified, ~4286 runs, 0 violations

## Phase 5 — Tez-Coder v0 🟦 (data ✅, training 🔒)

- [x] Training set v1 (small: 26 verified programs + 108 mutation repair traces, `data/v1/`); scale needs a model (`sample.mjs`)
- [x] Contamination check: `data/contamination.mjs` (equality, a 24-token common sequence, prompt text); 4 examples were removed from training because they overlapped with TezBench; enforced by a test
- [x] **Fine-tuning set v2** (2026-10-22): `data/v2/` — 8,322 chat examples (8,014 train / 308 validation, ≈ 3.9 M tokens): 3,166 pure-function, 1,603 web-app (timers, clock, fetch, JSON, save/restore, POST), 3,431 repair (compiler diagnostics → corrected program, 571 with prover errors) and 122 error-code explanations. Every program is proved and its tests run; an independent audit (`data/v2/audit.mjs`) re-proves all 2,747 answers, re-rejects all 3,371 broken programs and finds no TezBench/web-benchmark leak. The user runs the fine-tune (Unsloth); recipe in `data/v2/README.md`
- [ ] 🔒 A 3–4B fine-tune (LoRA), evaluation — needs a GPU and a base model; the data pipeline is ready (`data/`)
- [ ] 🔒 Open-weights + dataset release (after a licence check) — there are no weights; a release needs the user's approval

**Exit criterion:** Mode B pass@1 clearly above Mode A.

## Phase 6 — In-browser model (2 weeks)

- [ ] 🔒 A quantised Tez-Coder (q4) + a WebGPU runtime — the "Run the model in this browser" option in the Studio is written (`studio/src/localModel.ts`, Transformers.js 4.3.1, from the example on the model card; Gemma 4 E4B `q4f16`, ~4.9 GB); it builds and passes type checking but is **not tried end to end** (needs the download + WebGPU); also, an ONNX export of the Tez fine-tune does not exist yet (LoRA merge + ONNX conversion)
- [ ] 🔒 GBNF constrained generation **in the browser** — needs an in-browser model runtime. The part that can be done locally is complete: `tools/gbnf/` (a character-level Earley recognizer + an mlx-lm logits processor; `spec/tools/ebnf2gbnf.py --check` now verifies the generated GBNF itself on 44 examples with this recognizer). This check **caught a real bug**: the generated GBNF required a newline inside blocks (`{ 1 }` could not be derived) and the old check had not noticed because it tested the EBNF; fixed.
  Constrained decoding measurement (Gemma 4 E4B, greedy, TezBench 20 tasks): fine-tuned + no card: valid syntax 80% → **95%**, passes 70% → 70%; fine-tuned + card: 100% / 50% (unchanged); not fine-tuned + card: 100% / 55%. Conclusion: the grammar fixes syntax almost for free but does not raise success — the bottleneck is type/contract meaning, not syntax
- [ ] 🔒 An end-to-end demo without internet — depends on the above (the Studio already compiles, runs and verifies offline)

## Phase 7 — Effects (later)

- [x] `Sub.every(ms, msg)` + an optional `subs(model)`: time enters the pure `update` as an event; a duration ≥ 50 ms is proven in the compiler; the host starts/stops/rebuilds the timers according to the model; hot-swap also replays tick events (the traffic-light and countdown examples, verified in the browser)
- [ ] ⏸ `Cmd` (HTTP, storage) and a capability model — user decision: not in the first stage

---

## Phase 8 — Developer infrastructure for real projects (decision of 2026-10-15)

What is missing for someone who wants to develop with Tez in a terminal; in order:

- [x] **Multi-file / `use`**: `use "file.tez"` brings dependencies into one program; paths are relative and stay inside the project (`E_USE_NOT_FOUND`/`E_USE_CYCLE`/`E_USE_BAD_PATH`), a file is loaded once, names are unique in the program (no shadowing); each file sees only its own names and those of the files it `use`s directly (`E_USE_MISSING`, with a fix). All files are verified together; diagnostics point to the right file with the `file` field. Example: `spec/examples/projects/shop`
- [x] Ready binaries + the npm package **prepared, not published**: `release.yml` (5 platforms + an npm tarball on a `v*` tag → a DRAFT release), `npm/tez-lang` (the WASM compiler, the `tez` command, multi-file, Z3: the system `z3` or `z3-solver` WASM). Publishing = the user's approval (tag + `npm publish`)
- [x] CLI polish (2026-10-22): `tez --help`/`<command> --help` succeed and a bare `tez` is a usage error (exit 2); an unknown flag is rejected instead of ignored; `--watch` on `check`/`run`/`test`/`build` (and `site` in the npm edition) re-runs when any `.tez` file of the project changes; `tez new <dir> [--app]` starts a project from `spec/templates/`; `tez fmt <dir>` formats every file below a directory. Same behaviour in the native binary and the npm edition, both tested
- [ ] ⏸ VS Code extension / language server: postponed (user decision, 2026-10-22: not needed at this stage)
- [x] A multi-file project in the Studio (done in Phase 9)

## Phase 9 — Primary goal: live websites in the browser (reminder of 2026-10-15)

The real purpose: an LLM in the browser writes Tez, the code is compiled and run in the same browser, the site updates itself with timers/triggers (cron-like); the whole structure belongs to the web. Phase 8 supports this goal, it does not replace it. In order:

- [x] A multi-file project in the Studio: file tabs (+ file / delete), the project is stored in the browser, diagnostics point to the right file, run/test/App/hot-swap/Verify (Z3) over the whole project; example: Examples → shop (the `tez_web` project API: `check_project`, `verify_project_step`, `run_project`, `test_project`, `build_project`)
- [x] Export the site: the Studio **Publish** button and `tez site main.tez -o index.html` → a single file (wasm + runtime + app host inlined, strict CSP); runs on any static host or from disk. (Not yet: a shareable link/hosting)
- [x] Triggers: `Sub.every` (interval), `Sub.clock(ms, f)` (wall clock → cron-like scheduling as pure code), `Sub.online`, `Sub.visible`; the event log records the argument too (hot-swap replays it). Example 55. The card is exactly 1,200 tokens (at the limit)
- [x] Persistence and network: `save`/`restore` (local storage; a restored model is built with verified code) and `fetches` → `Fetch.get` (default: only the page's own origin, extra origins with `tez site --allow=`; no cookies/redirects, 10 s, 1 MB). Example 56. Not yet: WebSocket
- [~] The in-browser LLM → compile → **prove** → run loop: "Ask a model" in the Studio now feeds back not only type errors but Z3 proof errors (with the counterexample) to the model; an accepted app runs automatically (verified end to end with a fake OpenAI-compatible server: an unproven division was rejected, the fixed version ran). **Tried with a real model (2026-10-22, mlx-serve, Gemma 4 E4B 4-bit):** untuned, it does not write Tez web pages: 0% accepted without an example, 19% with a complete example app in the prompt (`bench/tezbench/web.mjs`, 8 held-out page tasks × 2 samples, 2 turns); it invents JSX-like syntax. The Studio now adds the example app to web requests. Fine-tuning (dataset v2) is the fix. **Missing:** a trial with a WebGPU model in the browser (a 4.9 GB download; there is no ONNX export of a tuned model)

## Phase 10 — Gaps found by building a real site (2026-10-22)

The first independent site (`examples/hn-live`: the top five Hacker News stories, rebuilt by a cron job every 5 minutes and refreshed in the browser on the wall clock) worked, but only after cutting the JSON apart with string splits. Building it showed what the language and the host lacked. Fixed in this round:

- [x] **JSON**: `Json.parse` and `get/at/path/str/int/float/bool/list/members/isNull/toText` in the prelude, written in Tez (a pushdown reader, one token per step, `decreases` proved) and proved by the verifier (it caught a missing range proof for surrogate pairs). `\uXXXX` escapes and surrogate pairs work; example `57-json.tez`; its tests also run in compiled WebAssembly
- [x] **Requests**: `Fetch.json` (parse the answer), `Fetch.post`, `Fetch.postJson`, `Fetch.send(method, url, headers, body, ...)`; the host allows GET/POST/PUT/PATCH/DELETE, at most 10 headers (never cookies, `host`, `sec-*`), a 64 KB body, only allow-listed origins; a request is told apart by method, url and body
- [x] **Primitives and helpers**: `Str.toFloat`, `Char.fromCode`, `Str.replace`, `Str.indexOf`; `Time.hhmm/hms/date/weekday`; `Html.link/list/ordered`
- [x] **Methods on the prelude's own types** (`j.get("k")` on a `Json`); `tez site --css=FILE --title=TEXT`
- [x] The card stays 1,199/1,200 tokens: the app modules are documented in `spec/language.md` (a test requires it) and taught by the fine-tuning set instead

Still open (found, not done):

- [ ] Fan-out is manual: a list and then one request per element needs the index plumbing by hand (no `Fetch.all`)
- [ ] No WebSocket / server-sent events; no retry or per-request timeout; the 5-minute cron without an open page needs an external scheduler (the GitHub Actions workflow in `examples/hn-live/workflow`)
- [ ] `List.sort` sorts integers only (`sortBy` takes an integer key); no `Map`; no `Str.padLeft`; no regular expressions; no date parsing or arithmetic beyond writing dates
- [ ] `Json.parse` is quadratic in the number of tokens (fine for API answers, not for megabytes); numbers keep their text
- [ ] The Studio's Publish button has no CSS/title/allow options (only `tez site` does)
- [ ] A program's own types cannot have methods (only the prelude's)

## Standing rules

- Every syntax change: `spec/` → `tez_fmt` → the `card.md` budget → re-measure TezBench (the [`add-syntax-feature`](../.agents/skills/add-syntax-feature/SKILL.md) skill).
- Every new error class: a test that rejects **and** one that accepts (the [`add-diagnostic`](../.agents/skills/add-diagnostic/SKILL.md) skill).
- Soundness: everything that cannot be proven is rejected. There is no "accept for now".
