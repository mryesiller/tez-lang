---
name: add-diagnostic
description: Procedure for adding a new compiler error/diagnostic code, including its JSON shape, fix suggestions and the mandatory reject/accept tests.
---
# Add a diagnostic

Diagnostics are the feedback channel for LLMs, so wording and structure are product features.

1. **Code.** Add a variant to `Code` in `crates/tez_diag/src/lib.rs` with a stable `E_<AREA>_<NAME>` string. Codes are never renamed or reused; deprecate instead.
2. **Message.** One short, templated sentence, no prose. State the problem, not the history.
3. **Payload.** Fill `expected` when the compiler needed something specific. Fill `counterexample` for verifier failures. Add a `fixes` entry whenever a safe, mechanical edit exists; edits use `span` + `replace` (empty span = insertion).
4. **Recovery.** The reporting phase must continue after this error so a single run reports every independent problem.
5. **Tests.**
   - a **must-reject** program that triggers exactly this code at the expected span,
   - a **must-accept** nearby program that must not trigger it,
   - a JSON snapshot of the report.
6. **Docs.** Add the code to the table in `docs/ARCHITECTURE.md` §10 and the error-class table in §5 if it adds a guarantee.
7. **Data.** Tell `dataset-engineer`: the new code needs repair-triple coverage.

Gate: `make check`.
