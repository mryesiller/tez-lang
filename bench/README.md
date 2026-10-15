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
- `tezbench/` — TezBench: 20 tasks, each with a Tez reference, a TypeScript reference and language-neutral cases.
  `node tezbench/run.mjs --self-check` proves the references pass; `--candidates DIR` judges `DIR/<id>.tez|.ts`
  on four levels (parses, type-checks, verifies, passes the cases); `--prompts DIR [--card]` writes the prompts to
  give a model (with the language card for Mode B). Results are written with `--label` to `results/`.
  These tasks must never enter training data (`data/` checks for overlap).
