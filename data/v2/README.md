# Tez fine-tuning set v2

A supervised fine-tuning set for teaching a small model (3B–4B) to write Tez: **8,322 chat examples (8,014 train, 308 validation), about 3.9 million tokens.**
Fine-tuning is done by you; this directory only holds the data and the tools that make and audit it.

Every program in it was **proved by the Tez compiler** (types, contracts, indexing, division, termination) and its **tests were run**.
Nothing was written down that the compiler did not accept, and every "broken" program in a repair example is rejected by the compiler.

## Files

| File | What |
|---|---|
| `train.jsonl`, `valid.jsonl` | One example per line: `{"messages": [{"role": "user", ...}, {"role": "assistant", ...}]}`. Use these. |
| `train.meta.jsonl`, `valid.meta.jsonl` | Same order: `{id, kind, family, program}`. For analysis only, never for training. |
| `REPORT.json` | Counts per kind and family, the mistakes used in repair examples, rejected candidates, filtered rows. |
| `AUDIT.json` | Result of the independent audit (below) and the length statistics. |
| `train.alpaca.jsonl`, `valid.alpaca.jsonl` | **Not committed.** `node data/v2/build.mjs --alpaca` writes `{"instruction", "input", "output"}` rows if your trainer wants Alpaca. |

The split is by program: a validation program never appears in training, and neither do its repair examples.

## What is in it

| Kind | Rows | The user asks | The assistant answers |
|---|---:|---|---|
| `pure` | 3,166 | for a function, in three phrasings (a TezBench-style "Task / Signature" prompt, a named-function prompt, a one-line prompt); a few ask for a program of 2–3 functions (683 programs) | one `tez` block: the proved function(s), sometimes with a few `test` items |
| `app` | 1,603 | for a web page in words ("a countdown that stops by itself", "a page that reloads status.txt every 30 seconds", ...) | a complete app: `init`, `update`, `view`, and when the page needs them `subs`, `fetches`, `save`/`restore`, plus tests |
| `repair` | 3,431 | to fix a program the compiler rejected, shown with the compiler's real diagnostics (code, line, message, expected form, counterexample, fixes) exactly as the repair loop prints them | the complete corrected program |
| `explain` | 122 | what a compiler error code means (64 codes, two phrasings; from `spec/errors.md`) | the meaning and how to fix it |

- **Pure functions** come from 78 families (arithmetic, lists, text, `Option`/`Result`, reading JSON, dates and times, text replace/search, records, variants, refinement types, higher-order functions, tuples, recursion with `decreases`, and a group that needs contracts: safe indexing, safe division, termination). Each task is generated with random parameters and names, and has test cases computed independently in JavaScript; a task whose cases fail is dropped.
- **Apps** come from 49 families (JSON feeds that reload themselves, a weather card, a feedback form that POSTs, a date-and-time clock, counters with limits, votes, theme switch, forms and converters, timers driven by `Sub.every`, clocks driven by `Sub.clock`, online/visible badges, tabs, lists, pagination, progress bars, polls, flashcards, sign-in validation, fetch-on-open, fetch every N seconds, "reload when the minute changes", status page, saved note/preferences). Half of the variants of 13 of them also come with their own `save`/`restore`, which the compiler proves keeps the model's refinements.
- **Repairs** break a proved program in one targeted way (24 kinds: dropped match arm or `else`, wrong type, unknown name or constructor, missing record field, function-style method call, mixed `&&`/`||`, snake_case, shadowing, and the ones only the prover catches: dropped or weakened `requires`, wrong zero check, off-by-one guard, constant `decreases`, too-strong `ensures`) and keep the example only when the compiler really rejects the result. 25 distinct diagnostic codes occur; 571 examples carry a verification error (`E_REFINEMENT`, `E_DIV_ZERO`, `E_TERMINATION`, `E_INDEX_BOUNDS`, `E_PRECONDITION`, `E_POSTCONDITION`). No single kind of mistake is more than 14% of the repairs.
- **The 41 example programs** of `spec/examples` are included too (prompt: their title).
- 6% of the prompts carry the language card (`spec/card.md`, 1,198 tokens) so that the tuned model still reads one when it is given; the rest need no card, which is the point.

## Held out (never trained on)

- The 20 **TezBench** tasks (`bench/tezbench/tasks.mjs`): every program is checked for a 24-token overlap with a reference solution and every prompt for the task sentence; 32 programs were dropped. Several families are built to stay away from them (no sum-to-n, abs-diff, largest, count-positive, reverse text, clamp-score, factorial, fibonacci, primality, sum of squares, keep-evens, double-all, join-words, word-count, capitalize, average, last-or, gcd, power, zip-sum).
- The 8 **web tasks** of `bench/tezbench/web.mjs` (`webtasks.mjs`): counter with reset, show/hide paragraph, Celsius box, seconds-since-open stopwatch, morning/afternoon greeting, button that loads quote.txt, to-do list with Remove, remembered visit count. The app families are other ideas; the counter and tabs programs in `spec/examples` are in the set, as they are the reference examples.

Use those two benchmarks to judge the tuned model. Tuning on them would make the numbers meaningless.

## Check it yourself

```bash
cargo build --release -p tez-lang
TEZ_BIN=target/release/tez node data/v2/audit.mjs
```

The audit re-proves all 2,747 distinct answers, re-rejects all 3,371 distinct broken programs, looks for TezBench/web-benchmark leaks and train/validation overlap, checks that no (prompt, answer) pair repeats and measures lengths. It exits 1 on any problem; the committed `AUDIT.json` is from the build in this directory.

Rebuild (deterministic for a given `--seed`; about 2.5 minutes): `TEZ_BIN=target/release/tez node data/v2/build.mjs --seed 1 --pure 80 --apps 60`.

## Using it with Unsloth

The chat format works with any chat template. A sketch for a 3B instruct model (adapt the model name and template):

```python
from unsloth import FastLanguageModel
from unsloth.chat_templates import get_chat_template, train_on_responses_only
from datasets import load_dataset
from trl import SFTTrainer, SFTConfig

model, tokenizer = FastLanguageModel.from_pretrained("unsloth/Llama-3.2-3B-Instruct", max_seq_length=2560, load_in_4bit=True)
model = FastLanguageModel.get_peft_model(model, r=32, lora_alpha=32, lora_dropout=0,
    target_modules=["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"])
tokenizer = get_chat_template(tokenizer, chat_template="llama-3.1")      # the template of the model you chose

ds = load_dataset("json", data_files={"train": "data/v2/train.jsonl", "validation": "data/v2/valid.jsonl"})
ds = ds.map(lambda ex: {"text": tokenizer.apply_chat_template(ex["messages"], tokenize=False)})

trainer = SFTTrainer(model=model, tokenizer=tokenizer, train_dataset=ds["train"], eval_dataset=ds["validation"],
    args=SFTConfig(dataset_text_field="text", max_seq_length=2560, per_device_train_batch_size=4, gradient_accumulation_steps=4,
                   num_train_epochs=2, learning_rate=2e-4, lr_scheduler_type="cosine", warmup_ratio=0.03,
                   eval_strategy="steps", eval_steps=100, logging_steps=10, output_dir="tez-lora"))
trainer = train_on_responses_only(trainer, instruction_part="<|start_header_id|>user<|end_header_id|>\n\n",
                                  response_part="<|start_header_id|>assistant<|end_header_id|>\n\n")
trainer.train()
```

Notes: only 1 of 8,322 examples is longer than 2,048 tokens and 1,004 are longer than 1,024 (estimated; see `AUDIT.json`), so `max_seq_length` 2,560 loses nothing. Train on the **answers only** (`train_on_responses_only`): the prompts, and especially the repair prompts with their diagnostics, are inputs. Two epochs is a sensible start (about 7.9 million trained tokens); watch the validation loss.
A code-heavy base model (for example a Qwen2.5-Coder 3B) will likely adapt fastest; Tez reads like Rust/Elm/Kotlin.

## After tuning: measure

```bash
# held-out TezBench, no card needed (T=0.7, three samples, as in docs/DEVELOPMENT.md)
node bench/tezbench/generate.mjs --base-url http://127.0.0.1:PORT/v1 --model YOUR-MODEL --out OUT --modes tez-nocard --temp 0.7
node bench/tezbench/run.mjs --candidates OUT/tez-nocard --label tuned-s1
# held-out web pages: does it write an app the compiler accepts?
node bench/tezbench/web.mjs --base-url http://127.0.0.1:PORT/v1 --model YOUR-MODEL --example none --samples 3 --label tuned
```

For reference, an untuned Gemma 4 E4B (4-bit) with the language card scores 46% on TezBench and 0% on the web tasks (19% when a complete example app is added to the prompt).

## Limits to know about

- The programs are **template-generated** with random names and parameters (78 pure and 49 app families, plus combinations), so the variety is real but bounded: the model will see the same shapes many times. Held-out tasks of a new shape are the real test. Add families to `pure.mjs` / `apps.mjs` for new shapes; every family is verified the same way.
- English prompts only. Single-file programs only (no `use`).
- The explain rows repeat the catalog text; they teach the vocabulary, not reasoning.
- A passing audit says the *programs* are correct and proved. Whether a web page looks right is not checked, only that its `update` and `view` behave as its tests say.

## Licence

Generated by this repository's tools from its own examples: MIT OR Apache-2.0, like the rest of the repository.
