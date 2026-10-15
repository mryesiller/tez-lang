// Shared helpers for the data pipeline.
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

export const root = new URL("../", import.meta.url).pathname;
export const bin = process.env.TEZ_BIN ?? `${root}target/debug/tez`;
const dir = mkdtempSync(join(tmpdir(), "tez-data-"));
let n = 0;

function withFile(source, f) {
  const file = join(dir, `d${n++}.tez`);
  writeFileSync(file, source);
  return f(file);
}

/** Diagnostics for a program ([] = accepted). `verify:false` stops after the type checker. */
export function diagnose(source, { verify = true } = {}) {
  return withFile(source, (file) => {
    const r = spawnSync(bin, ["check", "--json", ...(verify ? [] : ["--no-verify"]), file], { encoding: "utf8", timeout: 60000 });
    try {
      return JSON.parse(r.stdout).diagnostics ?? [];
    } catch {
      return [{ code: "E_INTERNAL", message: (r.stderr || r.stdout).slice(0, 200) }];
    }
  });
}

/** Canonical formatting, or null when the source does not parse. */
export function canonical(source) {
  return withFile(source, (file) => {
    const r = spawnSync(bin, ["fmt", file], { encoding: "utf8" });
    return r.status === 0 ? r.stdout : null;
  });
}

/** Normal form used to compare programs: canonical text with comments and blank lines removed. */
export function normalize(source) {
  const c = canonical(source) ?? source;
  return c
    .split("\n")
    .filter((l) => l.trim() && !l.trim().startsWith("//"))
    .map((l) => l.trim().replace(/\s+/g, " "))
    .join("\n");
}

export function tokens(source) {
  return normalize(source).match(/[A-Za-z_][A-Za-z0-9_]*|\d+|"[^"]*"|\S/g) ?? [];
}

export function jsonl(rows) {
  return rows.map((r) => JSON.stringify(r)).join("\n") + (rows.length ? "\n" : "");
}
