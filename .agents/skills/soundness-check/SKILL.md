---
name: soundness-check
description: Adversarially try to make the Tez compiler accept a program that violates a guarantee. Use after any change to type checking, verification, the solver, or error recovery.
---
# Soundness check

Goal: find a program the compiler accepts that can still reach a guaranteed-away error.

For each error class in `docs/ARCHITECTURE.md` §5 touched by the change:
1. Write 5 minimal programs that **should be rejected** (boundary values, empty list, zero, negative, max-size, off-by-one in `decreases`, shadowed contract, contract on a lambda, error-recovery path).
2. Run `tez check --json` on each. Any `status: ok` is a **soundness bug**: stop and report the program.
3. Probe failure paths: solver `unknown`, timeout, cache hit with a changed formula, parser recovery that drops a node. All must end in rejection.
4. Add every program that exposed a bug (after the fix) to `crates/*/tests/reject/`.
5. Check the other direction cheaply: the change must not reject the examples in `spec/examples/`.

Report: classes covered, programs tried, result per program. "No bug found" is only valid with the list of what was tried.
