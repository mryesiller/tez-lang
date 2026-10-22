#!/usr/bin/env node
// Builds the fine-tuning set v2: programs the compiler has proved (and whose tests pass), repair examples made from
// them, and the example programs of the repository. Nothing is written that the compiler did not accept.
//
//   TEZ_BIN=target/release/tez node data/v2/build.mjs [--out data/v2] [--seed 1] [--pure 40] [--apps 30] [--repairs 2]
//
// Output (all JSONL, UTF-8):
//   train.jsonl / valid.jsonl            {"messages": [{"role":"user",...}, {"role":"assistant",...}]}   (chat format)
//   train.alpaca.jsonl / valid.alpaca.jsonl   {"instruction", "input", "output"}      (Alpaca format, only with --alpaca)
//   train.meta.jsonl / valid.meta.jsonl  one line per row: {id, kind, family, program}                    (not for training)
//   REPORT.json                           counts, rejection reasons, contamination drops
import { spawn } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { webTasks } from "../../bench/tezbench/webtasks.mjs";
import { renderDiagnostics } from "../../tools/agent/loop.mjs";
import { overlaps } from "../contamination.mjs";
import { root } from "../lib.mjs";
import { apps } from "./apps.mjs";
import { families } from "./pure.mjs";
import { rng } from "./rng.mjs";
import { PROOF_OPERATORS, appOperators, pureOperators } from "./mutate.mjs";

const a = process.argv.slice(2);
const opt = (k, d) => (a.includes(k) ? a[a.indexOf(k) + 1] : d);
const bin = process.env.TEZ_BIN ?? `${root}target/release/tez`;
const out = opt("--out", `${root}data/v2`);
const seed = Number(opt("--seed", "1"));
const pureAttempts = Number(opt("--pure", "40"));
const appAttempts = Number(opt("--apps", "30"));
const repairsPer = Number(opt("--repairs", "4"));
const opCap = Number(opt("--op-cap", "220")); // no single kind of mistake may dominate the repair examples
const jobs = Number(opt("--jobs", "8"));
const alpacaToo = a.includes("--alpaca");
const only = opt("--only", null); // a family id prefix, for debugging
const work = mkdtempSync(join(tmpdir(), "tez-v2-"));
let fileNo = 0;

// ------------------------------------------------------------------------------------------ running the compiler
function run(args, input) {
  return new Promise((resolve) => {
    const p = spawn(bin, args, { stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    const timer = setTimeout(() => p.kill(), 120000);
    p.stdout.on("data", (d) => (stdout += d));
    p.stderr.on("data", (d) => (stderr += d));
    p.on("close", (code) => {
      clearTimeout(timer);
      resolve({ code, stdout, stderr });
    });
    void input;
  });
}
function fileFor(text) {
  const f = join(work, `p${fileNo++}.tez`);
  writeFileSync(f, text);
  return f;
}
/** Canonical formatting, or null when the text does not parse. */
async function fmt(text) {
  const r = await run(["fmt", fileFor(text)]);
  return r.code === 0 ? r.stdout : null;
}
/** Proves the program and runs its tests. Resolves `{ok, why}`. */
async function proveAndTest(text) {
  const r = await run(["test", fileFor(text)]);
  if (r.code === 0) return { ok: true };
  const why = (r.stderr || r.stdout).split("\n").find((l) => /error\[|FAILED|ERROR/.test(l)) ?? (r.stderr || r.stdout).slice(0, 120);
  return { ok: false, why: why.trim().slice(0, 160) };
}
/** Diagnostics of a (possibly broken) program: [] when it is accepted. */
async function diagnose(text) {
  const r = await run(["check", "--json", fileFor(text)]);
  try {
    return JSON.parse(r.stdout).diagnostics ?? [];
  } catch {
    return [{ code: "E_INTERNAL", message: (r.stderr || r.stdout).slice(0, 100) }];
  }
}
async function pool(items, n, f) {
  const results = new Array(items.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: n }, async () => {
      while (next < items.length) {
        const i = next++;
        results[i] = await f(items[i], i);
      }
    }),
  );
  return results;
}

// ------------------------------------------------------------------------------------------ prompts
const card = readFileSync(`${root}spec/card.md`, "utf8").trim();
const fence = (code) => `\`\`\`tez\n${code.trim()}\n\`\`\``;

function purePrompt(t, r, style) {
  const types = t.types ? `\n\nThe types are:\n${fence(t.types)}` : "";
  let body;
  if (style === 0) {
    body = `Write the following function in Tez. Reply with Tez code only, no explanation.\n\nTask: ${t.desc}${types}\n\nSignature:\n${t.sig}\n`;
  } else if (style === 1) {
    body = `Write a Tez function named \`${t.name}\`. ${t.desc}${types}\n\nIts signature is:\n${fence(t.sig)}`;
  } else {
    body = `In Tez: ${t.desc} Name the function \`${t.name}\`.${t.types ? ` Use these types, declared in the same program:\n${fence(t.types)}` : ""}`;
  }
  return withCard(body, r);
}
function appPrompt(t, r, style) {
  const styles = [
    `Write a Tez web app (it defines init, update and view). ${t.desc}`,
    `Build this page as a Tez web app.\n\n${t.desc}`,
    `${t.desc}\n\nWrite it as a Tez app with init, update and view. Reply with Tez code only.`,
    `Make a small interactive web page in Tez. ${t.desc}`,
  ];
  return withCard(styles[style % styles.length], r);
}
/** A small share of the prompts carries the language card, so a tuned model still reads one when it is given. */
function withCard(body, r) {
  return r.chance(0.06) ? `Write a Tez program. Reply with Tez code only, in one code block.\n\n${card}\n\n${body}` : body;
}

// ------------------------------------------------------------------------------------------ generation
const rejected = {};
const note = (family, why) => ((rejected[family] ??= {})[why] = (rejected[family]?.[why] ?? 0) + 1);

function candidates() {
  const list = [];
  const gen = (fs, attempts, kind) => {
    for (const f of fs) {
      if (only && !f.id.startsWith(only)) continue;
      const seen = new Set();
      for (let i = 0; i < attempts; i++) {
        const r = rng(seed * 1000003 + [...f.id].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7) + i * 7919);
        let t;
        try {
          t = f.make(r);
        } catch (e) {
          note(f.id, `generator: ${String(e.message).slice(0, 60)}`);
          continue;
        }
        if (seen.has(t.code)) continue;
        seen.add(t.code);
        list.push({ family: f.id, kind, task: t, r: rng(seed + i * 104729 + list.length) });
      }
    }
  };
  gen(families, pureAttempts, "pure");
  gen(apps, appAttempts, "app");
  return list;
}

/** `n` distinct style numbers out of `of` */
const pickStyles = (r, of, n) => r.shuffle(Array.from({ length: of }, (_, i) => i)).slice(0, n);

const testName = (name, i) => `${name.replace(/[^A-Za-z0-9]/g, "")}Example${i + 1}`;
const testLines = (name, cases) => cases.map(([call, want], i) => `test ${testName(name, i)} = ${call} == ${want}`);

async function buildProgram(c) {
  const { task: t } = c;
  if (c.kind === "app") {
    const text = await fmt(t.code);
    if (text === null) return note(c.family, "does not parse"), null;
    const v = await proveAndTest(text);
    if (!v.ok) return note(c.family, v.why), null;
    return { ...c, completion: text, prompts: () => pickStyles(c.r, 4, 2).map((i) => appPrompt(t, c.r, i)) };
  }
  // a pure function: all cases must pass, but only some of the tests are shown in the answer
  const all = await fmt(`${t.code}\n\n${testLines(t.name, t.cases).join("\n")}\n`);
  if (all === null) return note(c.family, "does not parse"), null;
  const v = await proveAndTest(all);
  if (!v.ok) return note(c.family, v.why), null;
  const withTests = c.r.chance(0.3);
  const shown = withTests ? `${t.code}\n\n${testLines(t.name, t.cases.slice(0, c.r.int(2, 3))).join("\n")}\n` : `${t.code}\n`;
  const completion = await fmt(shown);
  if (completion === null) return note(c.family, "does not parse (answer)"), null;
  return { ...c, completion, prompts: () => pickStyles(c.r, 3, 2).map((i) => purePrompt(t, c.r, i)) };
}

// ------------------------------------------------------------------------------------------ main
const t0 = Date.now();
mkdirSync(out, { recursive: true });
const cands = candidates();
console.error(`${cands.length} candidates (${families.length} pure families, ${apps.length} app families)`);

let done = 0;
const built = (await pool(cands, jobs, async (c) => {
  const r = await buildProgram(c);
  if (++done % 200 === 0) console.error(`  ${done}/${cands.length} checked, ${((Date.now() - t0) / 1000).toFixed(0)} s`);
  return r;
})).filter(Boolean);
console.error(`${built.length} programs accepted`);

// the example programs of the repository (accepted ones), with a title as the prompt
const exDir = `${root}spec/examples`;
for (const f of readdirSync(exDir).filter((f) => f.endsWith(".tez") && !f.endsWith(".reject.tez")).sort()) {
  const text = await fmt(readFileSync(`${exDir}/${f}`, "utf8"));
  if (text === null) continue;
  if (!(await proveAndTest(text)).ok) continue;
  const title = f.replace(/\.tez$/, "").replace(/^\d+-/, "").replace(/-/g, " ");
  const isApp = /^fn view\(/m.test(text);
  built.push({ family: "example", kind: isApp ? "app" : "pure", task: { name: title }, completion: text, prompts: () => [isApp ? `Write a Tez web app (it defines init, update and view): ${title}.` : `Write a Tez program: ${title}.`], r: rng(1) });
}

// ------------------------------------------------------------------------------------------ programs of several functions
// Each function was proved on its own; a program that holds two or three of them is proved again as a whole.
const comboCount = Number(opt("--combos", "700"));
const singles = built.filter((p) => p.kind === "pure" && p.family !== "example" && p.task.cases);
const comboCands = [];
const rc = rng(seed + 4242);
for (let i = 0; i < comboCount * 2 && comboCands.length < comboCount; i++) {
  const k = rc.chance(0.6) ? 2 : 3;
  const picks = [];
  for (let j = 0; j < 40 && picks.length < k; j++) {
    const c = rc.pick(singles);
    const clash = picks.some((x) => x.family === c.family || x.task.name === c.task.name || (x.task.types && c.task.types && x.task.types.split(" ")[1] === c.task.types.split(" ")[1]));
    if (!clash) picks.push(c);
  }
  if (picks.length < k) continue;
  if (picks.filter((p) => /^fn pad\(/m.test(p.task.code)).length > 1) continue;
  const names = picks.map((p) => p.task.name);
  if (new Set(names).size !== names.length) continue;
  const key = picks.map((p) => p.task.code).sort().join("|");
  if (comboCands.some((c) => c.key === key)) continue;
  comboCands.push({ key, picks, r: rng(seed + i * 31337) });
}
let comboDone = 0;
const combos = (await pool(comboCands, jobs, async (c) => {
  const code = c.picks.map((p) => p.task.code).join("\n\n");
  const tests = c.picks.flatMap((p) => testLines(p.task.name, p.task.cases));
  const all = await fmt(`${code}\n\n${tests.join("\n")}\n`);
  if (all === null) return note("combo", "does not parse"), null;
  const v = await proveAndTest(all);
  if (!v.ok) return note("combo", v.why), null;
  const withTests = c.r.chance(0.3);
  const shown = withTests ? `${code}\n\n${c.picks.flatMap((p) => testLines(p.task.name, p.task.cases.slice(0, 2))).join("\n")}\n` : `${code}\n`;
  const completion = await fmt(shown);
  if (completion === null) return null;
  const item = (p, i) => `${i + 1}. ${p.task.desc}${p.task.types ? `\n   Types: ${p.task.types}` : ""}\n   Signature: ${p.task.sig.split("\n").join("\n   ")}`;
  const intro = c.r.pick(["Write a Tez program with the following functions.", "Write these functions in one Tez program. Reply with Tez code only.", "Implement all of the following in Tez (one file, no explanation)."]);
  const prompt = `${intro}\n\n${c.picks.map(item).join("\n\n")}`;
  if (++comboDone % 100 === 0) console.error(`  ${comboDone} combos`);
  return { family: "combo", kind: "pure", task: { name: names(c), desc: `several functions: ${c.picks.map((p) => p.task.name).join(", ")}` }, completion, prompts: () => [prompt], r: c.r };
})).filter(Boolean);
function names(c) {
  return c.picks.map((p) => p.task.name).join("+");
}
console.error(`${combos.length} multi-function programs`);
built.push(...combos);

// ------------------------------------------------------------------------------------------ contamination and dedupe
const heldOutPrompts = webTasks.map((w) => w.prompt.toLowerCase());
const stats = { dropped_overlap_tezbench: 0, dropped_overlap_webbench: 0, duplicates: 0 };
const seenCode = new Set();
const programs = [];
for (const p of built) {
  const key = p.completion.replace(/\s+/g, " ");
  if (seenCode.has(key)) {
    stats.duplicates++;
    continue;
  }
  seenCode.add(key);
  if (overlaps(p.completion).length) {
    stats.dropped_overlap_tezbench++;
    continue;
  }
  p.promptList = p.prompts();
  if (p.promptList.some((q) => heldOutPrompts.some((h) => q.toLowerCase().includes(h)))) {
    stats.dropped_overlap_webbench++;
    continue;
  }
  programs.push(p);
}
programs.forEach((p, i) => (p.id = `${p.family}#${i}`));
console.error(`${programs.length} programs after filters`, stats);

// ------------------------------------------------------------------------------------------ repairs
/** True when the mutation changed a `test` line: breaking the tests teaches nothing about the program. */
function breaksATest(original, broken) {
  const tests = (text) => text.split(/\n\s*\n/).filter((blk) => /^test /.test(blk.trim())).join("\n\n");
  return tests(original) !== tests(broken);
}
const repairRows = [];
const repairStats = { tried: 0, kept: 0 };
const shuffled = rng(seed + 99).shuffle(programs.filter((p) => p.family !== "example" || true));
const jobsList = [];
for (const p of shuffled) {
  const ops = p.kind === "app" ? appOperators : pureOperators;
  const muts = [];
  for (const [operator, op] of Object.entries(ops)) {
    for (const broken of op(p.completion)) if (broken !== p.completion && !breaksATest(p.completion, broken)) muts.push({ p, operator, broken });
  }
  // one mutation per kind first, so that the kinds are spread evenly over the programs
  const rr = rng(seed + p.id.length * 31 + muts.length);
  const byOp = {};
  for (const m of rr.shuffle(muts)) (byOp[m.operator] ??= []).push(m);
  const order = rr.shuffle(Object.keys(byOp));
  const picked = [];
  for (let round = 0; picked.length < repairsPer + 3; round++) {
    let any = false;
    for (const o of order) if (byOp[o][round] && picked.length < repairsPer + 3) (picked.push(byOp[o][round]), (any = true));
    if (!any) break;
  }
  picked.forEach((m) => jobsList.push(m));
}
console.error(`${jobsList.length} mutations to try`);
const mutated = await pool(jobsList, jobs, async (m) => {
  repairStats.tried++;
  const diagnostics = await diagnose(m.broken);
  return diagnostics.length && !diagnostics.some((d) => d.code === "E_INTERNAL") ? { ...m, diagnostics } : null;
});
const perProgram = new Map();
const perOperator = {};
for (const m of mutated.filter(Boolean)) {
  const n = perProgram.get(m.p.id) ?? 0;
  if (n >= repairsPer || (perOperator[m.operator] ?? 0) >= (PROOF_OPERATORS.has(m.operator) ? opCap * 2 : opCap)) continue;
  perProgram.set(m.p.id, n + 1);
  perOperator[m.operator] = (perOperator[m.operator] ?? 0) + 1;
  repairStats.kept++;
  const task = m.p.family === "combo" ? m.p.promptList[0].split("\n\n").slice(1).join("\n\n") : m.p.task.desc ?? `Write the program ${m.p.task.name}.`;
  const head = "Your Tez program was rejected by the compiler. Fix every problem and reply with the complete corrected program in one code block.";
  const prompt = `${head}\n\nTask: ${task.trim()}\n\nYour program:\n\`\`\`\n${m.broken.trim()}\n\`\`\`\n\nCompiler diagnostics:\n${renderDiagnostics(m.diagnostics)}`;
  repairRows.push({ id: `${m.p.id}~${m.operator}`, kind: "repair", family: `repair.${m.operator}`, program: m.p.id, prompt, completion: m.p.completion, codes: [...new Set(m.diagnostics.map((d) => d.code))] });
}
console.error(`${repairRows.length} repair examples`, repairStats);

// ------------------------------------------------------------------------------------------ split and write
// what the compiler's messages mean (from the error catalog): one answer per code, asked in two ways
const explainRows = [];
const catalog = readFileSync(`${root}spec/errors.md`, "utf8").split("\n");
for (const line of catalog) {
  const m = line.match(/^\| `(E_[A-Z_]+)` \| (.+?) \| (.+?) \|$/);
  if (!m) continue;
  const [, code, meaning, fix] = m;
  const answer = `\`${code}\` means: ${meaning.trim()}\n\nHow to fix it: ${fix.trim()}`;
  explainRows.push({ id: `explain#${code}#a`, kind: "explain", family: "explain", program: `explain#${code}`, prompt: `What does the Tez compiler error ${code} mean, and how do I fix it?`, answer });
  explainRows.push({ id: `explain#${code}#b`, kind: "explain", family: "explain", program: `explain#${code}`, prompt: `My Tez program is rejected with ${code}. What is wrong?`, answer });
}
const rows = [
  ...programs.flatMap((p) => p.promptList.map((prompt, i) => ({ id: `${p.id}/${i}`, kind: p.kind, family: p.family, program: p.id, prompt, completion: p.completion }))),
  ...repairRows,
  ...explainRows,
];
const splitOf = new Map();
const rs = rng(seed + 7);
for (const p of programs) splitOf.set(p.id, rs.next() < 0.04 ? "valid" : "train"); // by program: its repairs follow it
[...new Set(explainRows.map((e) => e.program))].forEach((id, i) => splitOf.set(id, i % 12 === 0 ? "valid" : "train"));
const bySplit = { train: [], valid: [] };
for (const r of rows) bySplit[splitOf.get(r.program) ?? "train"].push(r);
for (const s of ["train", "valid"]) {
  const list = rng(seed + (s === "train" ? 1 : 2)).shuffle(bySplit[s]);
  const reply = (r) => (r.answer ?? fence(r.completion));
  const chat = (r) => JSON.stringify({ messages: [{ role: "user", content: r.prompt }, { role: "assistant", content: reply(r) }] });
  const alpaca = (r) => JSON.stringify({ instruction: r.prompt, input: "", output: reply(r) });
  const meta = (r) => JSON.stringify({ id: r.id, kind: r.kind, family: r.family, program: r.program });
  writeFileSync(`${out}/${s}.jsonl`, list.map(chat).join("\n") + "\n");
  if (alpacaToo) writeFileSync(`${out}/${s}.alpaca.jsonl`, list.map(alpaca).join("\n") + "\n");
  writeFileSync(`${out}/${s}.meta.jsonl`, list.map(meta).join("\n") + "\n");
}
const count = (list, key) => list.reduce((m, r) => ((m[key(r)] = (m[key(r)] ?? 0) + 1), m), {});
const report = {
  built: new Date().toISOString(),
  seed,
  rows: { train: bySplit.train.length, valid: bySplit.valid.length },
  byKind: count(rows, (r) => r.kind),
  programs: programs.length,
  byFamily: count(programs, (r) => r.family),
  repairsByOperator: count(repairRows, (r) => r.family),
  repairDiagnosticCodes: count(repairRows.flatMap((r) => r.codes.map((c) => ({ c }))), (r) => r.c),
  ...stats,
  rejectedCandidates: rejected,
  seconds: Math.round((Date.now() - t0) / 1000),
};
writeFileSync(`${out}/REPORT.json`, JSON.stringify(report, null, 2) + "\n");
console.log(JSON.stringify({ rows: report.rows, byKind: report.byKind, programs: report.programs, seconds: report.seconds, ...stats }, null, 2));
