#!/usr/bin/env node
// node tools/agent/solve.mjs --task "..." --base-url http://localhost:11434/v1 --model gemma4:e4b [--no-card] [--turns 3]
import { readFileSync } from "node:fs";
import { cliChecker } from "./checker.mjs";
import { solve } from "./loop.mjs";
import { openAICompatible } from "./providers.mjs";

const root = new URL("../../", import.meta.url).pathname;
const a = process.argv.slice(2);
const opt = (n, d) => (a.includes(n) ? a[a.indexOf(n) + 1] : d);
const task = opt("--task");
const baseUrl = opt("--base-url", process.env.TEZ_LLM_URL);
const model = opt("--model", process.env.TEZ_LLM_MODEL);
if (!task || !baseUrl || !model) {
  console.error('usage: solve.mjs --task "<what to build>" --base-url <openai-compatible url> --model <name> [--no-card] [--turns N]\n(API key from TEZ_LLM_KEY)');
  process.exit(2);
}
const card = a.includes("--no-card") ? "" : readFileSync(`${root}spec/card.md`, "utf8");
const result = await solve({
  task,
  card,
  generate: openAICompatible({ baseUrl, model, apiKey: process.env.TEZ_LLM_KEY }),
  check: cliChecker(),
  maxTurns: Number(opt("--turns", "3")),
  onTurn: (t, i) => console.error(`turn ${i + 1}: ${t.diagnostics.length ? `${t.diagnostics.length} diagnostic(s), first ${t.diagnostics[0].code}` : "accepted"}`),
});
console.log(result.code);
process.exit(result.ok ? 0 : 1);
