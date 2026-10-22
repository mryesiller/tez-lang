#!/usr/bin/env node
// Audits the fine-tuning set without trusting the builder: re-proves every answer, checks that every broken program
// in a repair example is really rejected, looks for leaks (TezBench, the web benchmark, train/valid) and measures lengths.
//   TEZ_BIN=target/release/tez node data/v2/audit.mjs [--dir data/v2]        exit 1 on any failure
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { tasks } from "../../bench/tezbench/tasks.mjs";
import { webTasks } from "../../bench/tezbench/webtasks.mjs";
import { overlaps } from "../contamination.mjs";
import { root } from "../lib.mjs";

const a = process.argv.slice(2);
const dir = a.includes("--dir") ? a[a.indexOf("--dir") + 1] : `${root}data/v2`;
const bin = process.env.TEZ_BIN ?? `${root}target/release/tez`;
const work = mkdtempSync(join(tmpdir(), "tez-audit-"));
let n = 0;
const read = (f) => readFileSync(`${dir}/${f}`, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l));

function run(args) {
  return new Promise((resolve) => {
    const p = spawn(bin, args, { stdio: ["ignore", "pipe", "pipe"] });
    let out = "";
    p.stdout.on("data", (d) => (out += d));
    p.stderr.on("data", (d) => (out += d));
    p.on("close", (code) => resolve({ code, out }));
  });
}
const file = (text) => {
  const f = join(work, `a${n++}.tez`);
  writeFileSync(f, text);
  return f;
};
async function pool(items, k, f) {
  const res = new Array(items.length);
  let next = 0;
  await Promise.all(Array.from({ length: k }, async () => {
    while (next < items.length) {
      const i = next++;
      res[i] = await f(items[i]);
    }
  }));
  return res;
}
const code = (reply) => (/^```tez\n([\s\S]*?)\n```$/.exec(reply.trim()) ?? [])[1];

const problems = [];
const bad = (msg) => problems.push(msg);
const splits = {};
for (const s of ["train", "valid"]) {
  const alpacaFile = existsSync(`${dir}/${s}.alpaca.jsonl`);
  splits[s] = { chat: read(`${s}.jsonl`), meta: read(`${s}.meta.jsonl`), alpaca: alpacaFile ? read(`${s}.alpaca.jsonl`) : null };
  const { chat, meta, alpaca } = splits[s];
  if (chat.length !== meta.length || (alpaca && chat.length !== alpaca.length)) bad(`${s}: the files have different lengths`);
  chat.forEach((r, i) => {
    const m = r.messages;
    if (!Array.isArray(m) || m.length !== 2 || m[0].role !== "user" || m[1].role !== "assistant" || !m[0].content.trim() || !m[1].content.trim()) bad(`${s} row ${i}: malformed`);
    if (alpaca && (alpaca[i]?.instruction !== m?.[0]?.content || alpaca[i]?.output !== m?.[1]?.content)) bad(`${s} row ${i}: alpaca and chat differ`);
    if (meta[i]?.kind !== "explain" && code(m[1].content) === undefined) bad(`${s} row ${i}: the answer is not one tez code block`);
  });
}

// 1. every answer is proved and its tests pass; every broken program in a repair is rejected
const answers = new Map();
const broken = new Map();
for (const s of ["train", "valid"]) {
  const { chat, meta } = splits[s];
  chat.forEach((r, i) => {
    if (meta[i].kind === "explain") return;
    answers.set(code(r.messages[1].content), `${s}:${meta[i].id}`);
    if (meta[i].kind === "repair") {
      const m = /Your program:\n```\n([\s\S]*?)\n```\n\nCompiler diagnostics:/.exec(r.messages[0].content);
      if (!m) bad(`${s}:${meta[i].id}: no broken program in the prompt`);
      else broken.set(m[1], `${s}:${meta[i].id}`);
    }
  });
}
console.error(`re-proving ${answers.size} programs and re-rejecting ${broken.size} broken ones...`);
const proved = await pool([...answers], 8, async ([text, id]) => ({ id, r: await run(["test", file(`${text}\n`)]) }));
for (const p of proved) if (p.r.code !== 0) bad(`${p.id}: the answer does not pass \`tez test\`: ${p.r.out.split("\n")[0]}`);
const rejects = await pool([...broken], 8, async ([text, id]) => ({ id, r: await run(["check", "--no-verify", file(`${text}\n`)]) }));
// a broken program is rejected by the type checker or by the prover: try the prover for those that type-check
const need = rejects.filter((x) => x.r.code === 0);
const proofRejects = await pool(need, 8, async (x) => ({ id: x.id, r: await run(["check", file([...broken].find(([, id]) => id === x.id)[0] + "\n")]) }));
for (const x of proofRejects) if (x.r.code === 0) bad(`${x.id}: the "broken" program is accepted`);

// 2. leaks
const heldPrompts = [...tasks.map((t) => t.prompt), ...webTasks.map((w) => w.prompt)].map((p) => p.toLowerCase());
let leaks = 0;
for (const s of ["train", "valid"]) {
  splits[s].chat.forEach((r, i) => {
    const prompt = r.messages[0].content.toLowerCase();
    if (heldPrompts.some((h) => prompt.includes(h))) (leaks++, bad(`${s} row ${i}: the prompt contains a held-out task`));
  });
}
for (const [text, id] of answers) {
  const hit = overlaps(text);
  if (hit.length) (leaks++, bad(`${id}: overlaps TezBench ${hit.join(", ")}`));
}
const trainAnswers = new Set(splits.train.chat.map((r) => code(r.messages[1].content)).filter(Boolean));
let shared = 0;
for (const r of splits.valid.chat) if (trainAnswers.has(code(r.messages[1].content))) shared++;
if (shared) bad(`${shared} validation answers also appear in the training set`);
const pairs = new Set();
let dup = 0;
for (const r of [...splits.train.chat, ...splits.valid.chat]) {
  const k = `${r.messages[0].content}\u0000${r.messages[1].content}`;
  if (pairs.has(k)) dup++;
  pairs.add(k);
}
if (dup) bad(`${dup} exact duplicate (prompt, answer) pairs`);

// 3. lengths (Gemma 4 tokenizer when it is available; characters otherwise)
let count = (t) => Math.ceil(t.length / 3.2);
let unit = "tokens (estimated: characters / 3.2)";
const tokFile = `${root}bench/cache/gemma4/tokenizer.json`;
if (existsSync(tokFile)) {
  try {
    const { Tokenizer } = await import("@huggingface/tokenizers");
    const tok = new Tokenizer(JSON.parse(readFileSync(tokFile, "utf8")), {});
    count = (t) => tok.encode(t, { add_special_tokens: false }).ids.length;
    unit = "tokens (Gemma 4 tokenizer)";
  } catch {
    /* keep the estimate */
  }
}
const lengths = {};
for (const s of ["train", "valid"]) {
  splits[s].chat.forEach((r, i) => {
    const kind = splits[s].meta[i].kind;
    const t = count(r.messages[0].content) + count(r.messages[1].content);
    (lengths[kind] ??= []).push(t);
  });
}
const pct = (xs, p) => [...xs].sort((x, y) => x - y)[Math.min(xs.length - 1, Math.floor((p / 100) * xs.length))];
const summary = {};
for (const [k, xs] of Object.entries(lengths)) summary[k] = { rows: xs.length, mean: Math.round(xs.reduce((s, x) => s + x, 0) / xs.length), p95: pct(xs, 95), max: Math.max(...xs), over1024: xs.filter((x) => x > 1024).length, over2048: xs.filter((x) => x > 2048).length, total: xs.reduce((s, x) => s + x, 0) };
const all = Object.values(lengths).flat();
summary.all = { rows: all.length, mean: Math.round(all.reduce((s, x) => s + x, 0) / all.length), p95: pct(all, 95), max: Math.max(...all), over1024: all.filter((x) => x > 1024).length, over2048: all.filter((x) => x > 2048).length, total: all.reduce((s, x) => s + x, 0) };

const report = { ok: problems.length === 0, problems: problems.slice(0, 50), problemCount: problems.length, programsReproved: answers.size, brokenReRejected: broken.size, leaks, unit, lengths: summary };
writeFileSync(`${dir}/AUDIT.json`, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify(report, null, 2));
process.exit(report.ok ? 0 : 1);
