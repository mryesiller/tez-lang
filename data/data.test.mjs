import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { tasks } from "../bench/tezbench/tasks.mjs";
import { overlaps } from "./contamination.mjs";
import { diagnose, root } from "./lib.mjs";
import { operators, tracesFor } from "./mutate.mjs";

test("a TezBench reference solution is flagged, even reformatted", () => {
  const t = tasks.find((x) => x.id === "18-gcd");
  assert.deepEqual(overlaps(t.tez), ["18-gcd"]);
  assert.ok(overlaps(t.tez.replace(/\n  /g, "\n      ").replace("gcd", "gcd")).includes("18-gcd"));
});

test("a program that copies a task prompt is flagged", () => {
  assert.ok(overlaps(`// ${tasks[0].prompt}\nfn main() -> Int = 1`).includes(tasks[0].id));
});

test("an unrelated example is clean", () => {
  assert.deepEqual(overlaps(readFileSync(`${root}spec/examples/13-expression-tree.tez`, "utf8")), []);
});

test("mutations are rejected by the real compiler with a code", () => {
  const src = readFileSync(`${root}spec/examples/22-missing-arm.reject.tez`, "utf8");
  assert.ok(diagnose(src, { verify: false }).length > 0);
  const ok = readFileSync(`${root}spec/examples/06-shapes.tez`, "utf8");
  const traces = tracesFor(ok, { verify: false });
  assert.ok(traces.length > 0);
  for (const t of traces) assert.ok(t.diagnostics[0].code.startsWith("E_"), t.operator);
  assert.ok(Object.keys(operators).length >= 10);
});

test("the shipped v1 set contains no TezBench overlap and only accepted programs", () => {
  const rows = readFileSync(`${root}data/v1/programs.jsonl`, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l));
  assert.ok(rows.length >= 20);
  for (const r of rows) assert.deepEqual(overlaps(r.completion), [], r.id);
  const repairs = readFileSync(`${root}data/v1/repairs.jsonl`, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l));
  for (const r of repairs) assert.deepEqual(overlaps(r.fixed), [], r.id);
});

test("rejection sampling keeps accepted unique programs, drops contaminated ones, and records repairs", async () => {
  const { sample } = await import("./sample.mjs");
  const { cliChecker } = await import("../tools/agent/checker.mjs");
  const answers = [
    'fn main() -> Int = "no"', // rejected...
    "fn main() -> Int = 6 * 7", // ...repaired on the next turn -> accepted, with a repair trace
    "fn main() -> Int = 6 * 7", // duplicate
    tasks.find((t) => t.id === "18-gcd").tez, // contaminated
  ];
  let i = 0;
  const r = await sample([{ id: "t1", task: "answer" }], {
    generate: async () => answers[i++],
    check: cliChecker({ verify: false }),
    n: 3,
  });
  assert.equal(r.programs.length, 1);
  assert.equal(r.repairs.length, 1);
  assert.equal(r.repairs[0].diagnostics[0].code, "E_TYPE_MISMATCH");
  assert.equal(r.stats.duplicates, 1);
  assert.equal(r.stats.contaminated, 1);
});

test("the v2 fine-tuning set is well formed, split by program and free of held-out tasks", () => {
  const read = (f) => readFileSync(`${root}data/v2/${f}`, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l));
  const train = read("train.jsonl");
  const valid = read("valid.jsonl");
  const tm = read("train.meta.jsonl");
  const vm = read("valid.meta.jsonl");
  assert.equal(train.length, tm.length);
  assert.equal(valid.length, vm.length);
  assert.ok(train.length > 5000 && valid.length > 100);
  const code = (r) => /^```tez\n([\s\S]*?)\n```$/.exec(r.messages[1].content)?.[1];
  for (const r of [...train, ...valid]) {
    assert.deepEqual(r.messages.map((m) => m.role), ["user", "assistant"]);
    assert.ok(r.messages[0].content.trim() && r.messages[1].content.trim());
  }
  const kinds = new Set(tm.map((m) => m.kind));
  for (const k of ["pure", "app", "repair", "explain"]) assert.ok(kinds.has(k), k);
  // by program: no validation program (or one of its repairs) is in the training set
  const trainPrograms = new Set(tm.map((m) => m.program));
  for (const m of vm) assert.ok(!trainPrograms.has(m.program), m.program);
  // held-out prompts never appear in a prompt
  const held = [...tasks.map((t) => t.prompt)].map((p) => p.toLowerCase());
  for (const r of [...train, ...valid]) assert.ok(!held.some((h) => r.messages[0].content.toLowerCase().includes(h)));
  // a sample of the answers does not overlap a TezBench reference
  train.filter((_, i) => i % 60 === 0).forEach((r) => {
    const c = code(r);
    if (c) assert.deepEqual(overlaps(c), [], c.slice(0, 80));
  });
});
