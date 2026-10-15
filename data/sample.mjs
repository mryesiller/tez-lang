#!/usr/bin/env node
// Rejection sampling: task -> N candidates -> keep only programs the compiler accepts (verified) -> canonicalize
// -> dedupe -> drop anything overlapping TezBench. Failed candidates become repair traces when a later candidate
// (or repair turn) for the same task is accepted.
//
//   node data/sample.mjs --tasks tasks.jsonl --base-url URL --model M [--n 8] [--out data/out/run1]
//   tasks.jsonl: one {"id": "...", "task": "what to build"} per line
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { cliChecker } from "../tools/agent/checker.mjs";
import { firstPrompt, extractCode, repairPrompt } from "../tools/agent/loop.mjs";
import { openAICompatible } from "../tools/agent/providers.mjs";
import { overlaps } from "./contamination.mjs";
import { canonical, jsonl, root } from "./lib.mjs";

/**
 * @param {{id: string, task: string}[]} tasks
 * @param {{generate: (p: string) => Promise<string>, check: (c: string) => Promise<object[]>, card?: string, n?: number, repairTurns?: number}} o
 */
export async function sample(tasks, { generate, check, card = "", n = 4, repairTurns = 2 }) {
  const programs = [];
  const repairs = [];
  const stats = { candidates: 0, accepted: 0, duplicates: 0, contaminated: 0, repaired: 0 };
  const seen = new Set();
  for (const t of tasks) {
    for (let i = 0; i < n; i++) {
      let prompt = firstPrompt({ card, task: t.task });
      let code = extractCode(await generate(prompt));
      stats.candidates++;
      let diagnostics = await check(code);
      const failures = [];
      for (let turn = 0; diagnostics.length && turn < repairTurns; turn++) {
        failures.push({ code, diagnostics });
        prompt = repairPrompt({ card, task: t.task, code, diagnostics });
        code = extractCode(await generate(prompt));
        stats.candidates++;
        diagnostics = await check(code);
      }
      if (diagnostics.length) continue;
      const text = canonical(code);
      if (text === null) continue;
      const hit = overlaps(text);
      if (hit.length) {
        stats.contaminated++;
        continue;
      }
      if (seen.has(text)) {
        stats.duplicates++;
        continue;
      }
      seen.add(text);
      stats.accepted++;
      programs.push({ id: t.id, prompt: t.task, completion: text });
      for (const f of failures) {
        stats.repaired++;
        repairs.push({ id: t.id, operator: "model", code: f.code, diagnostics: f.diagnostics.map(({ code, message, span, expected }) => ({ code, message, span, expected })), fixed: text });
      }
    }
  }
  return { programs, repairs, stats };
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const a = process.argv.slice(2);
  const opt = (k, d) => (a.includes(k) ? a[a.indexOf(k) + 1] : d);
  const tasksFile = opt("--tasks");
  const baseUrl = opt("--base-url", process.env.TEZ_LLM_URL);
  const model = opt("--model", process.env.TEZ_LLM_MODEL);
  if (!tasksFile || !baseUrl || !model) {
    console.error("usage: sample.mjs --tasks tasks.jsonl --base-url URL --model M [--n 8] [--out DIR] [--no-card]");
    process.exit(2);
  }
  const tasks = readFileSync(tasksFile, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l));
  const card = a.includes("--no-card") ? "" : readFileSync(`${root}spec/card.md`, "utf8");
  const out = opt("--out", `${root}data/out/${new Date().toISOString().slice(0, 10)}`);
  mkdirSync(out, { recursive: true });
  const r = await sample(tasks, {
    generate: openAICompatible({ baseUrl, model, apiKey: process.env.TEZ_LLM_KEY, temperature: 0.8 }),
    check: cliChecker(),
    card,
    n: Number(opt("--n", "4")),
  });
  writeFileSync(`${out}/programs.jsonl`, jsonl(r.programs));
  writeFileSync(`${out}/repairs.jsonl`, jsonl(r.repairs));
  console.log(JSON.stringify(r.stats, null, 2));
}
