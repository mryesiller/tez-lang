// The prelude's trusted contracts must hold in the JavaScript runtime too (compiled programs use it, not the
// interpreter). Each `test` of crates/tez_interp/tests/prelude_contracts.tez becomes a function, is compiled to
// WebAssembly with the `tez` CLI, and must return true.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { load } from "./tez-runtime.mjs";

const root = new URL("../", import.meta.url).pathname;
const bin = process.env.TEZ_BIN ?? `${root}target/debug/tez`;

test("trusted prelude contracts hold in the WebAssembly runtime", async () => {
  const src = readFileSync(`${root}crates/tez_interp/tests/prelude_contracts.tez`, "utf8");
  const names = [...src.matchAll(/^test (\w+) =/gm)].map((m) => m[1]);
  assert.ok(names.length >= 16);
  const asFns = src.replace(/^test (\w+) =/gm, "fn $1() -> Bool =");
  const dir = mkdtempSync(join(tmpdir(), "tez-contracts-"));
  writeFileSync(join(dir, "c.tez"), asFns);
  const r = spawnSync(bin, ["build", "--no-verify", join(dir, "c.tez"), "-o", join(dir, "c.wasm")], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const rt = await load(readFileSync(join(dir, "c.wasm")));
  for (const n of names) assert.equal(rt.run(n), "true", `prelude contract ${n} is violated in the runtime`);
});

// The JSON reader, the time helpers and the text helpers are Tez code in the prelude, so they run in compiled
// WebAssembly as well; every `test` of the JSON example must come out the same there as in the interpreter.
test("the JSON example's tests hold in compiled WebAssembly", async () => {
  const src = readFileSync(`${root}spec/examples/57-json.tez`, "utf8");
  const names = [...src.matchAll(/^test (\w+) =/gm)].map((m) => m[1]);
  assert.ok(names.length >= 8);
  const asFns = src.replace(/^test (\w+) =/gm, "fn $1() -> Bool =");
  const dir = mkdtempSync(join(tmpdir(), "tez-json-"));
  writeFileSync(join(dir, "j.tez"), asFns);
  const r = spawnSync(bin, ["build", "--no-verify", join(dir, "j.tez"), "-o", join(dir, "j.wasm")], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const rt = await load(readFileSync(join(dir, "j.wasm")));
  for (const n of names) assert.equal(rt.run(n), "true", `${n} differs in WebAssembly`);
});
