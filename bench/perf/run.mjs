#!/usr/bin/env node
// Runtime speed: the same five workloads compiled by Tez (WebAssembly, run by runtime/tez-runtime.mjs) and written
// by hand in JavaScript (Node). Also times `tez check` (verification) on the example apps.
//   TEZ_BIN=target/release/tez node bench/perf/run.mjs [--json]
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { load } from "../../runtime/tez-runtime.mjs";

const here = new URL(".", import.meta.url).pathname;
const root = `${here}../../`;
const bin = process.env.TEZ_BIN ?? `${root}target/release/tez`;
const dir = mkdtempSync(join(tmpdir(), "tez-perf-"));
const best = (f, n = 5) => Math.min(...Array.from({ length: n }, () => { const t = performance.now(); f(); return performance.now() - t; }));

const js = {
  fib: () => { const fib = (n) => (n < 2 ? n : fib(n - 1) + fib(n - 2)); return fib(30); },
  sum: () => Array.from({ length: 1e6 }, (_, i) => i).map((x) => x * 2).filter((x) => x % 3 === 0).reduce((a, x) => a + x, 0),
  sort: () => Array.from({ length: 1e5 }, (_, i) => (i * 7919) % 100003).sort((a, b) => a - b)[50000],
  str: () => Array.from({ length: 5e4 }, (_, i) => String(i)).join(",").split(",").length,
  json: () => JSON.parse(`[${Array.from({ length: 300 }, (_, i) => `{"id":${i},"title":"story ${i}","score":${i * 3}}`).join(",")}]`).length,
};

const rows = [];
for (const name of Object.keys(js)) {
  const wasm = join(dir, `${name}.wasm`);
  const b = spawnSync(bin, ["build", "--no-verify", `${here}${name}.tez`, "-o", wasm], { encoding: "utf8" });
  if (b.status !== 0) throw new Error(`build ${name}: ${b.stderr}`);
  const rt = await load(readFileSync(wasm));
  const tez = best(() => rt.run("main"));
  rows.push({ name, tezMs: +tez.toFixed(1), nodeMs: +best(js[name]).toFixed(1) });
}

const checks = ["spec/examples/57-json.tez", "examples/hn-live/main.tez"].map((f) => {
  spawnSync(bin, ["check", `${root}${f}`]); // fills the proof cache
  const t = (env) => best(() => spawnSync(bin, ["check", `${root}${f}`], { env: { ...process.env, ...env } }), 3);
  return { file: f, warmCacheMs: +t({}).toFixed(0), noCacheMs: +t({ TEZ_NO_CACHE: "1" }).toFixed(0) };
});

if (process.argv.includes("--json")) console.log(JSON.stringify({ rows, checks }, null, 2));
else {
  console.log("workload   tez wasm    node");
  for (const r of rows) console.log(`${r.name.padEnd(8)} ${String(r.tezMs).padStart(8)} ms ${String(r.nodeMs).padStart(7)} ms`);
  console.log("\n`tez check` (verification): warm proof cache / no cache");
  for (const c of checks) console.log(`${c.file.padEnd(30)} ${String(c.warmCacheMs).padStart(5)} ms / ${String(c.noCacheMs).padStart(5)} ms`);
}
