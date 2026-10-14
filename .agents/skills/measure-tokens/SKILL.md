---
name: measure-tokens
description: Measure how many tokens Tez code, keywords and the language card cost in real tokenizers of several model families. Use whenever a syntax or size claim is made.
---
# Measure tokens

Never estimate. Count with real tokenizers.

1. Read the tokenizer set in `bench/tokens/tokenizers.toml` (fixed in Phase 0; ≥ 4 families incl. small open models). Pin versions.
2. **Keywords/operators:** every candidate must encode to exactly one token in every tokenizer, with and without a leading space and after a newline. Report failures per tokenizer.
3. **Programs:** count tokens for each `spec/examples/*.tez` and for its TypeScript twin under `bench/ts/`. Report the ratio Tez/TS per tokenizer; target ≤ 0.8 overall (≤ 0.9 at the end of Phase 0).
4. **Card:** count `spec/card.md`; hard cap 1,200 tokens in the largest tokenizer.
5. Save results as JSON in `bench/results/tokens-<date>.json`; do not overwrite earlier files.
6. Report a table (rows = tokenizer, columns = keywords failing / mean ratio / card tokens). Never average across tokenizers.

If `bench/tokens` does not exist yet, creating it is the first Phase 0 task: write it so one command reproduces the table.
