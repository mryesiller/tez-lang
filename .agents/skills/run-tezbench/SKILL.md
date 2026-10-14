---
name: run-tezbench
description: Run TezBench against a model and report syntax validity, pass@1, loop convergence and token ratio versus TypeScript. Use to evaluate any language or model change.
---
# Run TezBench

1. Pick the model, model (Gemma 4 E4B primary, one secondary small model, one frontier model), and mode: **A** card only, **B** fine-tuned, **C** in-browser quantized.
2. Run greedy and T=1.0/top_p=0.95/top_k=64, thinking off and on; fix seed; record model id, quantization and tokenizer versions.
3. For each task in `bench/tasks/`: build the prompt (card + task), generate, run `tez check --json`, run the task's tests.
   - Mode A also runs **with** and **without** GBNF constrained decoding.
   - Convergence: on failure feed the JSON diagnostics back, up to 3 turns.
4. Do the same tasks in TypeScript with identical models and prompts.
5. Report: syntax validity, pass@1, ≤3-turn convergence, token ratio, per model. Save JSON to `bench/results/<date>-<model>.json`.
6. Compare to the previous run. Any regression must be explained before the change that caused it merges.

Training data must never include TezBench tasks; if asked to, refuse and flag it.
