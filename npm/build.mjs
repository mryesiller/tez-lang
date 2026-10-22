// Builds the npm package: the compiler as WebAssembly (nodejs-friendly ESM) plus the runtime, into npm/tez-lang.
// Needs cargo (wasm32 target), wasm-bindgen 0.2.129 and wasm-opt (binaryen).  Run: node npm/build.mjs
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, renameSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const pkg = `${root}npm/tez-lang/`;
const run = (cmd, args) => execFileSync(cmd, args, { cwd: root, stdio: "inherit" });

run("cargo", ["build", "--release", "--target", "wasm32-unknown-unknown", "-p", "tez_web"]);
rmSync(`${pkg}wasm`, { recursive: true, force: true });
run("wasm-bindgen", ["--target", "web", "--out-dir", `${pkg}wasm`, "target/wasm32-unknown-unknown/release/tez_web.wasm"]);
run("wasm-opt", ["-Oz", "--enable-bulk-memory", "--enable-sign-ext", "--enable-nontrapping-float-to-int", "--enable-mutable-globals", "-o", `${pkg}wasm/opt.wasm`, `${pkg}wasm/tez_web_bg.wasm`]);
renameSync(`${pkg}wasm/opt.wasm`, `${pkg}wasm/tez_web_bg.wasm`);
rmSync(`${pkg}wasm/.gitignore`, { force: true });

mkdirSync(`${pkg}runtime`, { recursive: true });
for (const f of ["tez-runtime.mjs", "tez-app.mjs", "export-site.mjs"]) copyFileSync(`${root}runtime/${f}`, `${pkg}runtime/${f}`);
copyFileSync(`${root}spec/grammar.ebnf`, `${pkg}grammar.ebnf`);
mkdirSync(`${pkg}templates`, { recursive: true });
for (const f of ["program.tez", "app.tez"]) copyFileSync(`${root}spec/templates/${f}`, `${pkg}templates/${f}`);
for (const f of ["LICENSE-MIT", "LICENSE-APACHE"]) {
  try { copyFileSync(`${root}${f}`, `${pkg}${f}`); } catch { /* licence files are optional locally */ }
}
console.log("built npm/tez-lang");
