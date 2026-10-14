# bench/

Measurement, not opinion.

- `tokens/` — token-cost tool. `node measure.mjs` reports, against the primary tokenizer (Gemma 4):
  which keywords/operators are a single token where they are used, the token cost of every
  example next to its TypeScript twin (`ts/`), and the size of the language card.
  Setup: `pnpm install` in `bench/tokens`, and put the model's `tokenizer.json` in
  `bench/cache/gemma4/` (git-ignored; see `tokenizers.toml` for the pinned revision).
  `node measure.mjs --json` writes `results/tokens-<date>.json`.
- `ts/` — TypeScript twins of the examples (same behaviour, idiomatic style).
- `results/` — versioned JSON results; never overwritten.
- `tasks/` — TezBench tasks (Phase 3).
