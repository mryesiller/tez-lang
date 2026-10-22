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

import { spawn } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";

test("help succeeds; a missing command or a misspelt flag does not", () => {
  for (const args of [["--help"], ["help"], ["check", "--help"], ["new", "-h"]]) {
    const r = tez(args);
    assert.equal(r.status, 0, args.join(" "));
    assert.match(r.stdout, /USAGE:/);
  }
  const none = tez([]);
  assert.equal(none.status, 2);
  assert.match(none.stderr, /USAGE:/);
  const d = project({ "a.tez": "fn main() -> Int = 1\n" });
  for (const args of [["check", join(d, "a.tez"), "--jsno"], ["run", join(d, "a.tez"), "--check"], ["fmt", join(d, "a.tez"), "--watch"], ["new", "x", "--json"]]) {
    const r = tez(args);
    assert.equal(r.status, 2, args.join(" "));
    assert.match(r.stderr, /does not take/);
  }
});

test("new starts projects that check, test, run and are formatted", () => {
  for (const flag of [null, "--app"]) {
    const dir = join(mkdtempSync(join(tmpdir(), "tez-new-")), "app");
    const r = tez(["new", dir, ...(flag ? [flag] : [])]);
    assert.equal(r.status, 0, r.stderr);
    const main = join(dir, "main.tez");
    const check = tez(["check", main, "--no-verify"]);
    assert.equal(check.status, 0, check.stderr);
    assert.match(tez(["test", main, "--no-verify"]).stdout, /\d+ passed, 0 failed/);
    assert.equal(tez(["fmt", main, "--check"]).status, 0, "not formatted");
    if (!flag) assert.equal(tez(["run", main, "--no-verify"]).stdout.trim(), "20");
    // a second `new` into the same place refuses and leaves the files alone
    writeFileSync(main, `${readFileSync(main, "utf8")}// mine\n`);
    assert.equal(tez(["new", dir, ...(flag ? [flag] : [])]).status, 2);
    assert.ok(readFileSync(main, "utf8").endsWith("// mine\n"));
  }
});

test("fmt takes a directory", () => {
  const d = project({ "main.tez": 'use "lib/a.tez"\n\nfn main() -> Int = double(2)\n', "lib/a.tez": "fn double(n:Int)->Int=n*2\n", "notes.txt": "x\n" });
  assert.equal(tez(["fmt", d]).status, 2); // needs --check or --write
  const check = tez(["fmt", d, "--check"]);
  assert.equal(check.status, 1);
  assert.match(check.stderr, /a\.tez is not formatted/);
  assert.equal(tez(["fmt", d, "--write"]).status, 0);
  assert.equal(tez(["fmt", d, "--check"]).status, 0);
  assert.equal(readFileSync(join(d, "lib/a.tez"), "utf8"), "fn double(n: Int) -> Int = n * 2\n");
  const empty = mkdtempSync(join(tmpdir(), "tez-empty-"));
  assert.equal(tez(["fmt", empty, "--check"]).status, 2);
});

test("watch runs again when a file of the project changes", async () => {
  const d = project({ "main.tez": 'use "lib.tez"\nfn main() -> Int = one()\n', "lib.tez": "fn one() -> Int = 1\n" });
  const child = spawn("node", [bin, "check", join(d, "main.tez"), "--no-verify", "--watch"], { stdio: ["ignore", "pipe", "pipe"] });
  let out = "";
  let err = "";
  child.stdout.on("data", (c) => (out += c));
  child.stderr.on("data", (c) => (err += c));
  const until = async (f) => {
    for (let i = 0; i < 150 && !f(); i++) await new Promise((r) => setTimeout(r, 100));
    return f();
  };
  try {
    assert.ok(await until(() => out.includes("ok: ")), `no first run: ${out}${err}`);
    writeFileSync(join(d, "lib.tez"), 'fn one() -> Int = "x"\n'); // break the imported file, not the entry
    assert.ok(await until(() => err.includes("E_TYPE_MISMATCH")), `change not noticed: ${err}`);
    writeFileSync(join(d, "lib.tez"), "fn one() -> Int = 2\n");
    assert.ok(await until(() => out.split("ok: ").length > 2), "the fix was not noticed");
  } finally {
    child.kill();
  }
});
