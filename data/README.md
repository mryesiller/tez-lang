# data/

The training-data pipeline. Everything here is deterministic except the model you plug into `sample.mjs`.

| Step | Tool | Output |
|---|---|---|
| Accepted programs + fix traces from mutation (no model needed) | `node data/build-v1.mjs` | `v1/programs.jsonl`, `v1/repairs.jsonl`, `v1/REPORT.json` |
| Rejection sampling from a model | `node data/sample.mjs --tasks tasks.jsonl --base-url URL --model M` | `out/<date>/programs.jsonl`, `repairs.jsonl` (git-ignored) |
| Contamination check | `node data/contamination.mjs FILE.jsonl` | exit 1 if any row overlaps a TezBench task |

- A **program** row is `{id, prompt?, completion}`: only programs the compiler accepts (verified), in canonical form, unique.
- A **repair** row is `{id, operator, code, diagnostics, fixed}`: a rejected program, the compiler's stable diagnostics,
  and the accepted program. `operator` is the mutation that produced it (`drop-match-arm`, `unknown-name`, ...) or `model`.
- **TezBench never enters training data.** Every builder drops rows whose program equals a task's reference, shares a
  24-token run with it, or contains its prompt. `v1/REPORT.json` lists what was dropped (four examples overlap TezBench
  tasks: factorial, fibonacci, primes, gcd/power), and a test keeps the shipped set clean.
- v1 is small on purpose (examples only). Scale comes from `sample.mjs` with a model, then the same filters apply.

Tests: `node --test data/data.test.mjs`.
