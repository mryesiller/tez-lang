#!/usr/bin/env node
// Builds the chat-format SFT set for mlx-lm LoRA from data/v1 (examples + mutation repairs) and, when present,
// data/out/lora (rejection-sampled programs and model repairs). TezBench overlap is filtered again here.
//   node data/lora/prepare.mjs   ->   data/lora/{train,valid}.jsonl
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { repairPrompt } from "../../tools/agent/loop.mjs";
import { overlaps } from "../contamination.mjs";
import { root } from "../lib.mjs";

const read = (f) => (existsSync(f) ? readFileSync(f, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l)) : []);
const rows = [];
const title = (id) => id.replace(/^\d+-/, "").replace(/-/g, " ");

for (const r of read(`${root}data/v1/programs.jsonl`)) rows.push({ p: `Write a Tez program: ${title(r.id)}.`, a: r.completion, src: "example" });
for (const r of read(`${root}data/out/lora/programs.jsonl`)) rows.push({ p: `Write a Tez program. ${r.prompt}`, a: r.completion, src: "sampled" });
for (const f of ["data/v1/repairs.jsonl", "data/out/lora/repairs.jsonl"]) {
  for (const r of read(`${root}${f}`)) {
    rows.push({ p: repairPrompt({ card: "", task: "Fix the program.", code: r.code, diagnostics: r.diagnostics }), a: r.fixed, src: r.operator });
  }
}
const clean = rows.filter((r) => overlaps(r.a).length === 0);
// deterministic shuffle, then 10% validation
let seed = 12345;
const rnd = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
clean.sort(() => rnd() - 0.5);
const nValid = Math.max(8, Math.floor(clean.length / 10));
const toChat = (r) => JSON.stringify({ messages: [{ role: "user", content: r.p }, { role: "assistant", content: "```tez\n" + r.a.trim() + "\n```" }] });
writeFileSync(`${root}data/lora/valid.jsonl`, clean.slice(0, nValid).map(toChat).join("\n") + "\n");
writeFileSync(`${root}data/lora/train.jsonl`, clean.slice(nValid).map(toChat).join("\n") + "\n");
const by = {};
for (const r of clean) by[r.src === "example" || r.src === "sampled" ? r.src : "repair"] = (by[r.src === "example" || r.src === "sampled" ? r.src : "repair"] ?? 0) + 1;
console.log(`train ${clean.length - nValid}, valid ${nValid}; dropped ${rows.length - clean.length} for TezBench overlap; mix`, by);
