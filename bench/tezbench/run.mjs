// TezBench runner.
//   node run.mjs --self-check                   reference solutions must pass (tasks are valid)
//   node run.mjs --candidates DIR [--label L]   judge DIR/<task-id>.tez and DIR/<task-id>.ts
//   node run.mjs --prompts DIR [--card]         write each task's prompt to DIR/<id>.<lang>.prompt.txt
// A candidate gets four levels: parses -> type-checks -> verifies (Tez) -> passes every case.
// Results go to ../results/tezbench-<label>-<date>.json when --label is given.
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { prompt, tasks, tezTests, tsTests } from "./tasks.mjs";

const root = new URL("../../", import.meta.url).pathname;
const tez = process.env.TEZ_BIN ?? `${root}target/debug/tez`;
const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const opt = (name) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const work = mkdtempSync(join(tmpdir(), "tezbench-"));

function sh(cmd, argv, input) {
  const r = spawnSync(cmd, argv, { encoding: "utf8", input, timeout: 60000 });
  return { code: r.status ?? 1, out: `${r.stdout ?? ""}${r.stderr ?? ""}` };
}

/** Judges Tez source. Returns the highest level reached. */
function judgeTez(task, code) {
  const file = join(work, `${task.id}.tez`);
  writeFileSync(file, `${code.trim()}\n\n${tezTests(task)}\n`);
  const parse = sh(tez, ["check", "--no-verify", "--json", file]);
  let diags = [];
  try {
    diags = JSON.parse(parse.out).diagnostics ?? [];
  } catch {}
  const parses = !diags.some((d) => d.phase === "parse");
  const typed = parse.code === 0;
  if (!parses) return { level: "invalid", diag: diags[0]?.code };
  if (!typed) return { level: "parses", diag: diags[0]?.code };
  const verify = sh(tez, ["check", "--json", file]);
  if (verify.code !== 0) {
    let d;
    try {
      d = JSON.parse(verify.out).diagnostics?.[0]?.code;
    } catch {}
    return { level: "typed", diag: d };
  }
  const t = sh(tez, ["test", file]);
  return t.code === 0 ? { level: "passes" } : { level: "verified", diag: "TESTS_FAILED" };
}

function judgeTs(task, code) {
  const file = join(work, `${task.id}.ts`);
  writeFileSync(file, `${code.trim()}\n\n${tsTests(task)}\n`);
  const r = sh("node", ["--experimental-strip-types", "--no-warnings", file]);
  if (r.code === 0) return { level: "passes" };
  return { level: /SyntaxError/.test(r.out) ? "invalid" : "verified", diag: "TESTS_FAILED" };
}

const ORDER = ["invalid", "parses", "typed", "verified", "passes"];
const atLeast = (r, level) => ORDER.indexOf(r.level) >= ORDER.indexOf(level);

if (flag("--self-check")) {
  let bad = 0;
  for (const t of tasks) {
    const a = judgeTez(t, t.tez);
    const b = judgeTs(t, t.ts);
    const ok = a.level === "passes" && b.level === "passes";
    if (!ok) bad++;
    console.log(`${ok ? "ok    " : "FAILED"} ${t.id}  tez=${a.level}${a.diag ? `(${a.diag})` : ""}  ts=${b.level}`);
  }
  console.log(bad === 0 ? `all ${tasks.length} reference solutions pass` : `${bad} task(s) broken`);
  process.exit(bad === 0 ? 0 : 1);
}

if (opt("--prompts")) {
  const dir = opt("--prompts");
  mkdirSync(dir, { recursive: true });
  const card = flag("--card") ? readFileSync(`${root}spec/card.md`, "utf8") : "";
  for (const t of tasks) for (const lang of ["tez", "ts"]) writeFileSync(`${dir}/${t.id}.${lang}.prompt.txt`, prompt(t, lang, card));
  console.log(`wrote ${tasks.length * 2} prompts to ${dir}`);
  process.exit(0);
}

if (opt("--candidates")) {
  const dir = opt("--candidates");
  const rows = [];
  for (const t of tasks) {
    const row = { id: t.id };
    for (const lang of ["tez", "ts"]) {
      const p = `${dir}/${t.id}.${lang}`;
      row[lang] = existsSync(p) ? (lang === "tez" ? judgeTez : judgeTs)(t, readFileSync(p, "utf8")) : { level: "missing" };
    }
    rows.push(row);
  }
  const rate = (lang, level) => rows.filter((r) => atLeast(r[lang], level)).length / rows.length;
  const summary = {
    tez: Object.fromEntries(ORDER.slice(1).map((l) => [l, rate("tez", l)])),
    ts: { parses: rate("ts", "parses"), passes: rate("ts", "passes") },
  };
  console.log(rows.map((r) => `${r.id.padEnd(20)} tez=${r.tez.level.padEnd(9)} ts=${r.ts.level}`).join("\n"));
  console.log(`\nTez  pass@1 ${(summary.tez.passes * 100).toFixed(0)}%  verified ${(summary.tez.verified * 100).toFixed(0)}%  syntax-valid ${(summary.tez.parses * 100).toFixed(0)}%`);
  console.log(`TS   pass@1 ${(summary.ts.passes * 100).toFixed(0)}%`);
  const label = opt("--label");
  if (label) {
    const date = new Date().toISOString().slice(0, 10);
    const out = `${root}bench/results/tezbench-${label}-${date}.json`;
    writeFileSync(out, JSON.stringify({ label, date, tasks: tasks.length, summary, rows }, null, 2));
    console.log(`wrote ${out}`);
  }
  process.exit(0);
}

console.error("usage: node run.mjs --self-check | --candidates DIR [--label L] | --prompts DIR [--card]");
process.exit(2);
