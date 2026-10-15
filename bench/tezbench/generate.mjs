// Generates TezBench candidates with any OpenAI-compatible model server, in four modes:
//   tez-nocard   Tez prompt without the language card (baseline)
//   tez-card     Tez prompt with spec/card.md                       (Mode B)
//   tez-repair   tez-card, then up to 3 compiler-feedback turns     (Mode B + loop)
//   ts           the same task in TypeScript
// node generate.mjs --base-url http://127.0.0.1:1234/v1 --model google/gemma-4-e4b --out DIR [--modes a,b] [--temp 0]
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { cliChecker } from "../../tools/agent/checker.mjs";
import { extractCode, repairPrompt } from "../../tools/agent/loop.mjs";
import { openAICompatible } from "../../tools/agent/providers.mjs";
import { prompt, tasks } from "./tasks.mjs";

const root = new URL("../../", import.meta.url).pathname;
const a = process.argv.slice(2);
const opt = (k, d) => (a.includes(k) ? a[a.indexOf(k) + 1] : d);
const baseUrl = opt("--base-url", process.env.TEZ_LLM_URL);
const model = opt("--model", process.env.TEZ_LLM_MODEL);
const out = opt("--out");
if (!baseUrl || !model || !out) {
  console.error("usage: generate.mjs --base-url URL --model M --out DIR [--modes tez-nocard,tez-card,tez-repair,ts] [--temp 0]");
  process.exit(2);
}
const modes = opt("--modes", "tez-nocard,tez-card,tez-repair,ts").split(",");
const generate = openAICompatible({ baseUrl, model, apiKey: process.env.TEZ_LLM_KEY, temperature: Number(opt("--temp", "0")), maxTokens: Number(opt("--max-tokens", "1500")) });
const card = readFileSync(`${root}spec/card.md`, "utf8");
const check = cliChecker();

for (const mode of modes) {
  const dir = `${out}/${mode}`;
  mkdirSync(dir, { recursive: true });
  for (const t of tasks) {
    const lang = mode === "ts" ? "ts" : "tez";
    let code = extractCode(await generate(prompt(t, lang, mode === "tez-nocard" || mode === "ts" ? "" : card)));
    if (mode === "tez-repair") {
      for (let turn = 0; turn < 3; turn++) {
        const diagnostics = await check(code);
        if (diagnostics.length === 0) break;
        code = extractCode(await generate(repairPrompt({ card, task: `${t.prompt}\n\nSignature:\n${t.tezSig}`, code, diagnostics })));
      }
    }
    writeFileSync(`${dir}/${t.id}.${lang}`, code);
    console.error(`${mode} ${t.id}`);
  }
}
