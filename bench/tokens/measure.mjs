// Token measurements for Tez against real tokenizers (see tokenizers.toml; Gemma 4 is primary).
//   node measure.mjs            print the report
//   node measure.mjs --json     also write ../results/tokens-<date>.json
import { readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync } from "node:fs";
import { Tokenizer } from "@huggingface/tokenizers";

const root = new URL("../../", import.meta.url).pathname;
const cache = `${root}bench/cache/gemma4`;
if (!existsSync(`${cache}/tokenizer.json`)) {
  console.error(`missing ${cache}/tokenizer.json (download google/gemma-4-E4B tokenizer.json; see tokenizers.toml)`);
  process.exit(2);
}
const tok = new Tokenizer(JSON.parse(readFileSync(`${cache}/tokenizer.json`, "utf8")), {});
const count = (text) => tok.encode(text, { add_special_tokens: false }).ids.length;
const pieces = (text) => tok.encode(text, { add_special_tokens: false }).tokens;
const content = (text) => pieces(text).filter((t) => t.replace(/[▁\n\s]/g, "") !== "");

// ------------------------------------------------------------------ keywords and operators
const candidates = [
  "fn", "type", "const", "test", "use", "let", "match", "if", "else", "where", "with", "requires", "ensures",
  "decreases", "assert", "true", "false", "=>", "->", "==", "!=", "<=", ">=", "&&", "||", "++", "..",
  "List", "Option", "Result", "Some", "None", "Ok", "Err", "Int", "Float", "Bool", "Str", "Unit",
];
// where each token really appears in canonically formatted code
const contexts = {
  lineStart: (k) => `x\n${k}`,
  indented: (k) => `x\n  ${k}`,
  midLine: (k) => ` ${k}`,
};
const usedAt = (k) =>
  ["fn", "type", "const", "test", "use"].includes(k)
    ? ["lineStart"]
    : ["requires", "ensures", "decreases", "let", "assert"].includes(k)
      ? ["indented"]
      : ["midLine"];
const kw = candidates.map((k) => {
  const per = {};
  for (const [name, f] of Object.entries(contexts)) {
    const n = content(f(k)).length;
    per[name] = name === "midLine" ? n : n - 1; // minus the `x`
  }
  return { token: k, ...per, usedAt: usedAt(k), single: usedAt(k).every((c) => per[c] === 1) };
});
const multi = kw.filter((k) => !k.single);

// ------------------------------------------------------------------ programs
const examplesDir = `${root}spec/examples`;
const tsDir = `${root}bench/ts`;
const rows = [];
for (const f of readdirSync(examplesDir).filter((f) => f.endsWith(".tez") && !f.endsWith(".reject.tez")).sort()) {
  const tez = readFileSync(`${examplesDir}/${f}`, "utf8");
  const stem = f.replace(/\.tez$/, "");
  const tsPath = `${tsDir}/${stem}.ts`;
  const ts = existsSync(tsPath) ? readFileSync(tsPath, "utf8") : null;
  // the same program without the lines that exist only to be proved
  const code = tez.split("\n").filter((l) => !/^\s*(requires|ensures|decreases)\b/.test(l)).join("\n");
  rows.push({
    example: stem,
    tez: count(tez),
    tezCodeOnly: count(code),
    ts: ts ? count(ts) : null,
    ratio: ts ? +(count(tez) / count(ts)).toFixed(2) : null,
    ratioCodeOnly: ts ? +(count(code) / count(ts)).toFixed(2) : null,
  });
}
const paired = rows.filter((r) => r.ts);
const sum = (k) => paired.reduce((s, r) => s + r[k], 0);
const total = paired.length
  ? {
      tez: sum("tez"),
      tezCodeOnly: sum("tezCodeOnly"),
      ts: sum("ts"),
      ratio: +(sum("tez") / sum("ts")).toFixed(2),
      ratioCodeOnly: +(sum("tezCodeOnly") / sum("ts")).toFixed(2),
    }
  : null;

// ------------------------------------------------------------------ language card
const cardPath = `${root}spec/card.md`;
const card = existsSync(cardPath) ? count(readFileSync(cardPath, "utf8")) : null;
const CARD_BUDGET = 1200;

// ------------------------------------------------------------------ report
console.log(`tokenizer: gemma4 (primary), vocab ${tok.model?.vocab ? Object.keys(tok.model.vocab).length : "?"}`);
console.log(`\nkeywords/operators: ${kw.length - multi.length}/${kw.length} are a single token where they are used`);
for (const m of multi) console.log(`  NOT single: ${m.token}  ${JSON.stringify(m)}`);
console.log("\nexample                      tez   tez(code)   ts   ratio  ratio(code)");
for (const r of rows) {
  console.log(
    `${r.example.padEnd(26)} ${String(r.tez).padStart(5)} ${String(r.tezCodeOnly).padStart(9)} ${String(r.ts ?? "-").padStart(6)} ${String(r.ratio ?? "-").padStart(6)} ${String(r.ratioCodeOnly ?? "-").padStart(8)}`,
  );
}
if (total) console.log(`TOTAL (${paired.length} paired)         ${String(total.tez).padStart(5)} ${String(total.tezCodeOnly).padStart(9)} ${String(total.ts).padStart(6)} ${String(total.ratio).padStart(6)} ${String(total.ratioCodeOnly).padStart(8)}`);
if (card !== null) console.log(`\nlanguage card: ${card} tokens (budget ${CARD_BUDGET}) ${card <= CARD_BUDGET ? "OK" : "OVER BUDGET"}`);

if (process.argv.includes("--json")) {
  mkdirSync(`${root}bench/results`, { recursive: true });
  const out = `${root}bench/results/tokens-${new Date().toISOString().slice(0, 10)}.json`;
  writeFileSync(out, JSON.stringify({ tokenizer: "gemma4", keywords: kw, examples: rows, total, card, cardBudget: CARD_BUDGET }, null, 2) + "\n");
  console.log(`\nwrote ${out}`);
}
if (card !== null && card > CARD_BUDGET) process.exit(1);
