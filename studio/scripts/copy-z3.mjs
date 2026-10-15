// Z3 (WebAssembly) is large (33 MB) and loaded only when the user presses "Verify": serve it as static files.
import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
const build = join(dirname(require.resolve("z3-solver/package.json")), "build");
const out = new URL("../public/z3/", import.meta.url).pathname;
mkdirSync(out, { recursive: true });
for (const f of ["z3-built.js", "z3-built.wasm"]) {
  if (!existsSync(join(build, f))) throw new Error(`z3-solver/build/${f} is missing`);
  copyFileSync(join(build, f), join(out, f));
}
console.log("copied Z3 to public/z3/");
