import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const bin = fileURLToPath(new URL("../bin/tez.mjs", import.meta.url));
const tez = (args) => spawnSync("node", [bin, ...args], { encoding: "utf8" });

function project(files) {
  const dir = mkdtempSync(join(tmpdir(), "tez-"));
  for (const [name, text] of Object.entries(files)) {
    mkdirSync(join(dir, name, ".."), { recursive: true });
    writeFileSync(join(dir, name), text);
  }
  return dir;
}

const good = {
  "main.tez": 'use "lib/m.tez"\nfn main() -> Int = double(21)\ntest ok = double(2) == 4\n',
  "lib/m.tez": "fn double(x: Int) -> Int = x * 2\n",
};

test("--version and help", () => {
  assert.match(tez(["--version"]).stdout, /^tez \d+\.\d+\.\d+/);
  assert.equal(tez(["help"]).status, 0);
  assert.equal(tez(["nope"]).status, 2);
});

test("a multi-file project runs and tests", () => {
  const d = project(good);
  const r = tez(["run", join(d, "main.tez"), "--no-verify"]);
  assert.equal(r.stdout.trim(), "42");
  assert.match(tez(["test", join(d, "main.tez"), "--no-verify"]).stdout, /1 passed, 0 failed/);
});

test("errors name the dependency file and fail the exit code", () => {
  const d = project({ "main.tez": 'use "lib/m.tez"\nfn main() -> Int = f(1)\n', "lib/m.tez": 'fn f(x: Int) -> Int = "no"\n' });
  const r = tez(["check", join(d, "main.tez"), "--no-verify"]);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /lib\/m\.tez:1:/);
});

test("unprovable programs are rejected when a solver exists", (t) => {
  const d = project({ "main.tez": "fn f(x: Int) -> Int = 10 / x\nfn main() -> Int = f(1)\n" });
  const r = tez(["check", join(d, "main.tez")]);
  if (r.status === 2) return t.skip("no SMT solver on this machine");
  assert.equal(r.status, 1);
  assert.match(r.stderr, /E_DIV_ZERO/);
});

test("fmt prints the canonical form", () => {
  const d = project({ "f.tez": "fn  f( a:Int )->Int=a+1\n" });
  assert.equal(tez(["fmt", join(d, "f.tez")]).stdout, "fn f(a: Int) -> Int = a + 1\n");
});
