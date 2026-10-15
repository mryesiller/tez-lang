// Node-side checker: runs the `tez` binary and returns its diagnostics.
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const root = new URL("../../", import.meta.url).pathname;

export function tezBinary() {
  return process.env.TEZ_BIN ?? `${root}target/debug/tez`;
}

/** @returns {(code: string) => Promise<object[]>} */
export function cliChecker({ verify = true, bin = tezBinary() } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "tez-agent-"));
  let n = 0;
  return async (code) => {
    const file = join(dir, `p${n++}.tez`);
    writeFileSync(file, code);
    const r = spawnSync(bin, ["check", "--json", ...(verify ? [] : ["--no-verify"]), file], { encoding: "utf8" });
    try {
      return JSON.parse(r.stdout).diagnostics ?? [];
    } catch {
      return [{ code: "E_INTERNAL", message: `tez gave no JSON: ${(r.stderr || r.stdout).slice(0, 200)}`, span: { start: { line: 1, col: 1 } } }];
    }
  };
}
