#!/usr/bin/env node
// Build training set v1 from the accepted example programs (no model needed):
//   data/v1/programs.jsonl   {id, prompt?, completion}      accepted, canonical programs
//   data/v1/repairs.jsonl    {id, operator, code, diagnostics, fixed}   fix traces from mutation
// Programs that overlap a TezBench task are dropped (TezBench must never be trained on); the report lists them.
// Usage: node data/build-v1.mjs [--no-verify]
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { overlaps } from "./contamination.mjs";
import { canonical, diagnose, jsonl, root } from "./lib.mjs";
import { tracesFor } from "./mutate.mjs";

const verify = !process.argv.includes("--no-verify");
const exDir = `${root}spec/examples`;
const out = `${root}data/v1`;
mkdirSync(out, { recursive: true });

const programs = [];
const repairs = [];
const dropped = [];
const seen = new Set();

for (const f of readdirSync(exDir).filter((f) => f.endsWith(".tez") && !f.endsWith(".reject.tez")).sort()) {
  const id = f.replace(/\.tez$/, "");
  const text = canonical(readFileSync(`${exDir}/${f}`, "utf8"));
  if (text === null || diagnose(text, { verify }).length) {
    dropped.push({ id, why: "not accepted" });
    continue;
  }
  const hit = overlaps(text);
  if (hit.length) {
    dropped.push({ id, why: `overlaps TezBench ${hit.join(", ")}` });
    continue;
  }
  if (seen.has(text)) continue;
  seen.add(text);
  programs.push({ id, completion: text });
  for (const t of tracesFor(text, { verify })) {
    repairs.push({ id, operator: t.operator, code: t.broken, diagnostics: t.diagnostics.map(({ code, message, span, expected }) => ({ code, message, span, expected })), fixed: text });
  }
}

writeFileSync(`${out}/programs.jsonl`, jsonl(programs));
writeFileSync(`${out}/repairs.jsonl`, jsonl(repairs));
const byOp = {};
const byCode = {};
for (const r of repairs) {
  byOp[r.operator] = (byOp[r.operator] ?? 0) + 1;
  for (const d of r.diagnostics) byCode[d.code] = (byCode[d.code] ?? 0) + 1;
}
const report = { programs: programs.length, repairs: repairs.length, dropped, repairsByOperator: byOp, diagnosticCodes: byCode };
writeFileSync(`${out}/REPORT.json`, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ ...report, dropped: dropped.length, codes: Object.keys(byCode).length }, null, 2));
