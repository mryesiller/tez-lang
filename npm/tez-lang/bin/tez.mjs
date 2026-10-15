#!/usr/bin/env node
// The `tez` command (Node edition): same commands as the native binary, running the compiler as WebAssembly.
import { existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, basename, relative, sep } from "node:path";
import { open, TezError } from "../src/api.mjs";
import { siteHtml } from "../runtime/export-site.mjs";

const USAGE = `tez: a fast, verified language for LLMs and the browser

USAGE:
    tez check <file> [--json] [--no-verify]  type-check and verify a program
    tez run <file> [fn] [--no-verify] [--contracts]
                                            run a zero-argument function (default: main)
    tez test <file> [--no-verify] [--contracts]
                                            run the \`test\` items
    tez build <file> [-o out.wasm] [--no-verify]
                                            compile to a WebAssembly (WasmGC) module
    tez fmt <file> [--check | --write]      print / check / rewrite canonical formatting
    tez site <file> [-o index.html] [--no-verify]
                                            write one self-contained web page (program + runtime)
    tez grammar                             print the normative grammar (EBNF)
    tez --version

Programs are verified before they run. \`--no-verify\` skips verification (nothing is then
guaranteed); \`--contracts\` evaluates requires/ensures/refinements while running.
Verification needs an SMT solver: the \`z3\` executable if present, else the bundled WebAssembly Z3.
`;

const argv = process.argv.slice(2);
const flags = new Set(argv.filter((a) => a.startsWith("--")));
const pos = [];
let out = null;
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === "-o") out = argv[++i];
  else if (!argv[i].startsWith("--")) pos.push(argv[i]);
}
const cmd = argv[0]?.startsWith("-") && argv[0] !== "-o" ? argv[0] : (pos[0] ?? "");
const [file, ...rest] = cmd === pos[0] ? pos.slice(1) : [];

function fail(msg, code = 2) {
  console.error(`tez: ${msg}`);
  process.exit(code);
}

/** All `.tez` files under `dir` (the entry's directory), keyed by `/`-separated relative path. */
function projectFiles(dir) {
  const files = {};
  const walk = (d) => {
    for (const name of readdirSync(d)) {
      if (name === "node_modules" || name.startsWith(".")) continue;
      const p = join(d, name);
      const st = statSync(p);
      if (st.isDirectory()) walk(p);
      else if (name.endsWith(".tez")) files[relative(dir, p).split(sep).join("/")] = readFileSync(p, "utf8");
    }
  };
  walk(dir);
  return files;
}

async function main() {
  if (cmd === "--version" || cmd === "-V") {
    console.log(`tez ${(await open()).version()}`);
    return 0;
  }
  if (cmd === "" || cmd === "--help" || cmd === "-h" || cmd === "help") {
    process.stdout.write(USAGE);
    return 0;
  }
  if (cmd === "grammar") {
    process.stdout.write(readFileSync(new URL("../grammar.ebnf", import.meta.url), "utf8"));
    return 0;
  }
  if (!["check", "run", "test", "build", "site", "fmt"].includes(cmd)) {
    console.error(`tez: unknown command \`${cmd}\`\n\n${USAGE}`);
    return 2;
  }
  if (!file) fail(`\`${cmd}\` needs a file`);
  if (!existsSync(file)) fail(`cannot read \`${file}\`: no such file`);

  const tez = await open();
  if (cmd === "fmt") return fmt(tez);

  const root = dirname(file);
  const entry = basename(file);
  const dir = root === "." && !file.startsWith("./") ? "" : `${root}/`;
  const files = projectFiles(root);
  files[entry] = readFileSync(file, "utf8"); // the entry is always the file as given
  const json = flags.has("--json");

  const reject = (r) => {
    if (json) console.log(JSON.stringify(r.report, null, 2));
    else process.stderr.write(r.human ?? "");
    return 1;
  };

  // `check` and `run` prove the program first (as the native tool does); `test`/`build` also, unless --no-verify
  if (!flags.has("--no-verify")) {
    let v;
    try {
      v = await tez.verify(entry, files, dir);
    } catch (e) {
      fail(e.message);
    }
    if (!v.verified) return reject(v);
  }
  const parsed = tez.check(entry, files, dir);
  if (parsed.report.diagnostics.length) return reject(parsed);

  switch (cmd) {
    case "check":
      if (json) console.log(JSON.stringify(parsed.report, null, 2));
      else console.log(flags.has("--no-verify") ? `ok: ${file} (NOT verified: --no-verify)` : `ok: ${file}`);
      return 0;
    case "run": {
      const name = rest[0] ?? "main";
      if (flags.has("--contracts")) {
        const r = tez.run(entry, files, name, true, dir);
        if (!r.ok) return reject(r);
        console.log(r.value);
        return 0;
      }
      const b = tez.build(entry, files, dir);
      if (!b.ok) return b.report ? reject(b) : fail(`code generation failed: ${b.error}`, 1);
      try {
        console.log((await tez.load(b.wasm)).run(name));
        return 0;
      } catch (e) {
        const code = e instanceof TezError ? e.code : "E_INTERNAL";
        console.error(`error[${code}]: ${e.message}`);
        return 1;
      }
    }
    case "test": {
      const r = tez.test(entry, files, flags.has("--contracts"), dir);
      if (!r.ok) return reject(r);
      let failed = 0;
      for (const t of r.results) {
        if (t.outcome === "pass") console.log(`ok      ${t.name}`);
        else if (t.outcome === "fail") (failed++, console.log(`FAILED  ${t.name}  (${t.at})`));
        else (failed++, console.log(`ERROR   ${t.name}: ${t.message} [${t.code}]`));
      }
      console.log(`${r.results.length - failed} passed, ${failed} failed`);
      return failed ? 1 : 0;
    }
    case "site": {
      const b = tez.build(entry, files, dir);
      if (!b.ok) return b.report ? reject(b) : fail(`code generation failed: ${b.error}`, 1);
      const here = (f) => readFileSync(new URL(`../runtime/${f}`, import.meta.url), "utf8");
      const target = out ?? "index.html";
      writeFileSync(target, siteHtml({ title: basename(file, ".tez"), wasm: b.wasm, runtime: here("tez-runtime.mjs"), app: here("tez-app.mjs") }));
      console.log(`wrote ${target}: one self-contained page; serve it from any static host`);
      return 0;
    }
    case "build": {
      const b = tez.build(entry, files, dir);
      if (!b.ok) return b.report ? reject(b) : fail(`code generation failed: ${b.error}`, 1);
      const target = out ?? `${file.replace(/\.tez$/, "")}.wasm`;
      const bytes = Buffer.from(b.wasm, "base64");
      writeFileSync(target, bytes);
      console.log(`built ${target} (${bytes.length} bytes)`);
      return 0;
    }
  }

  function fmt(t) {
    const src = readFileSync(file, "utf8");
    const r = t.format(src);
    if (!r.ok) {
      if (r.comment) console.error(`tez: ${file}:${r.comment.line}:${r.comment.col}: a comment sits inside an expression; move it onto its own line before a statement`);
      else process.stderr.write(JSON.stringify(r.report));
      return 1;
    }
    if (flags.has("--check")) {
      if (r.text === src) return 0;
      console.error(`tez: ${file} is not formatted`);
      return 1;
    }
    if (flags.has("--write")) {
      if (r.text !== src) writeFileSync(file, r.text);
      return 0;
    }
    process.stdout.write(r.text);
    return 0;
  }
}

process.exitCode = await main();
