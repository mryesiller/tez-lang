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
