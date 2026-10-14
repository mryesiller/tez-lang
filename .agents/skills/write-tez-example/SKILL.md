---
name: write-tez-example
description: Write a canonical example Tez program for spec/examples/ with contracts, a TypeScript twin and expected output. Use when adding examples, benchmark tasks or training seeds.
---
# Write an example

Examples teach both humans and models, so they must be exemplary.

1. Choose one idea per file; 5–40 lines. Name it `NN-topic.tez` in `spec/examples/`.
2. Write it exactly as `tez fmt` would; no alternative spellings.
3. Give every public function a full signature and an `ensures` (and `requires`/`where` where meaningful). Teach the contract habit.
4. No dead features: use only constructs documented in `spec/card.md`.
5. Add `NN-topic.expected` (stdout or rendered view) and `bench/ts/NN-topic.ts` (idiomatic TypeScript twin, same behavior).
6. Add a must-reject variant `NN-topic.reject.tez` with the one change that should break a guarantee, and the diagnostic code it must produce.
7. Run `make check`; the example must parse, verify, run, and match `.expected`.

Examples are never edited to make a failing compiler pass; fix the compiler.
