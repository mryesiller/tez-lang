// A small benchmark for web pages (the primary goal): can a model, in the compile-prove-repair loop, write an app?
//   node web.mjs --base-url URL --model M [--samples 3] [--example none|counter|both] [--turns 3] [--max-tokens 800] [--label L]
// A run is "accepted" when the compiler proves the program (`tez check`, verification included) and it defines the
// app shape (`init`, `update`, `view`). It does not judge whether the page does what was asked: that needs a person.
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { cliChecker } from "../../tools/agent/checker.mjs";
import { solve } from "../../tools/agent/loop.mjs";
import { openAICompatible } from "../../tools/agent/providers.mjs";

const root = new URL("../../", import.meta.url).pathname;
const a = process.argv.slice(2);
const opt = (k, d) => (a.includes(k) ? a[a.indexOf(k) + 1] : d);
const baseUrl = opt("--base-url", process.env.TEZ_LLM_URL);
const model = opt("--model", process.env.TEZ_LLM_MODEL);
if (!baseUrl || !model) {
  console.error("usage: web.mjs --base-url URL --model M [--samples 3] [--example none|counter|both] [--turns 3] [--max-tokens 800] [--label L]");
  process.exit(2);
}

import { webTasks } from "./webtasks.mjs";

const card = readFileSync(`${root}spec/card.md`, "utf8");
const counter = readFileSync(`${root}spec/examples/15-counter-app.tez`, "utf8");
const samples = Number(opt("--samples", "3"));
const turns = Number(opt("--turns", "3"));
const label = opt("--label", "run");
const variants = opt("--example", "both") === "both" ? ["none", "counter"] : [opt("--example", "both")];
const generate = openAICompatible({ baseUrl, model, apiKey: process.env.TEZ_LLM_KEY, temperature: Number(opt("--temp", "0.7")), maxTokens: Number(opt("--max-tokens", "800")) });
const check = cliChecker();
const isApp = (code) => /^fn init\(/m.test(code) && /^fn update\(/m.test(code) && /^fn view\(/m.test(code);

async function gen(p) {
  for (let attempt = 1; ; attempt++) {
    try {
      return await generate(p);
    } catch (e) {
      if (attempt >= 6) throw e;
      console.error(`retry ${attempt}: ${String(e.message).slice(0, 80)}`);
      await new Promise((r) => setTimeout(r, 5000 * attempt));
    }
  }
}

const results = [];
for (const variant of variants) {
  for (const t of webTasks) {
    for (let s = 1; s <= samples; s++) {
      const r = await solve({ task: t.prompt, card, example: variant === "counter" ? counter : "", generate: gen, check, maxTurns: turns });
      const accepted = r.ok && isApp(r.code);
      const firstTurn = r.turns[0].diagnostics.length === 0 && isApp(r.turns[0].code);
      results.push({ variant, task: t.id, sample: s, accepted, firstTurn, turns: r.turns.length, firstError: r.turns[0].diagnostics[0]?.code ?? null, code: r.code });
      console.error(`${variant.padEnd(7)} ${t.id} #${s}: ${accepted ? `accepted in ${r.turns.length}` : `rejected (${r.turns.at(-1).diagnostics[0]?.code ?? "not an app"})`}`);
    }
  }
}

for (const variant of variants) {
  const rs = results.filter((r) => r.variant === variant);
  const pct = (n) => `${Math.round((100 * n) / rs.length)}%`;
  console.log(`${variant.padEnd(8)} accepted within ${turns} turns ${pct(rs.filter((r) => r.accepted).length)} · on the first try ${pct(rs.filter((r) => r.firstTurn).length)} (${rs.length} runs)`);
}
mkdirSync(`${root}bench/results`, { recursive: true });
const date = new Date().toISOString().slice(0, 10);
writeFileSync(`${root}bench/results/webbench-${label}-${date}.json`, JSON.stringify({ model, samples, turns, results }, null, 2));
