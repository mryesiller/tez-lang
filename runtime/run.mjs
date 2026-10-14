// Usage: node runtime/run.mjs program.wasm [function ...]
// For each zero-argument function (default `main`) prints one line: `name<TAB>value` or
// `name<TAB>ERROR <code>`. Exit code 0 unless the module itself cannot be loaded.
import { readFileSync } from "node:fs";
import { load, TezError } from "./tez-runtime.mjs";

const [file, ...names] = process.argv.slice(2);
let rt;
try {
  rt = await load(readFileSync(file));
} catch (e) {
  console.error(e);
  process.exit(2);
}
for (const name of names.length ? names : ["main"]) {
  try {
    console.log(`${name}\t${rt.run(name)}`);
  } catch (e) {
    if (e instanceof TezError) console.log(`${name}\tERROR ${e.code}`);
    else console.log(`${name}\tERROR E_INTERNAL ${e && e.message}`);
  }
}
