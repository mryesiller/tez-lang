---
name: add-syntax-feature
description: Procedure for adding or changing any Tez syntax or language feature. Use before touching the grammar, the formatter, or the language card.
---
# Add or change a language feature

A feature is accepted only if it improves measured LLM accuracy or token cost without breaking the card budget.

1. **Justify.** Write the problem in two sentences and the smallest syntax that solves it. List alternatives you rejected. Prefer a construct from TypeScript/Rust/Kotlin/Elm.
2. **Token check.** Run `measure-tokens` on the new keywords/operators. Reject any that is not one token in the primary tokenizer (Gemma 4); note secondary-tokenizer results.
3. **Grammar.** Edit `spec/grammar.ebnf` (normative). It must stay LL(1). Regenerate `spec/grammar.gbnf`; never hand-edit it.
4. **Formatter.** Define the single canonical layout in `tez_fmt`. If two spellings remain valid, remove one.
5. **Card budget.** Update `spec/card.md`; it must stay ≤ 1,200 tokens (measure it). Over budget → cut something or drop the feature.
6. **Examples.** Add at least two programs to `spec/examples/` (use `write-tez-example`), one showing the feature in a realistic task.
7. **Rejections.** Add must-reject cases for every misuse; add the diagnostic via `add-diagnostic`.
8. **Measure.** Re-run `run-tezbench` on Gemma 4 E4B. A drop in syntax validity or pass@1 beyond noise reverts the change.
9. **Record.** Note the decision and rejected alternatives in `docs/ARCHITECTURE.md` (decisions table) and tick the roadmap.

Done when: grammar, formatter, card, examples, tests and benchmark all agree.
