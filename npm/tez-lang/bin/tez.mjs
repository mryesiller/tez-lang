#!/usr/bin/env node
// The `tez` command (Node edition): same commands as the native binary, running the compiler as WebAssembly.
import { existsSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, basename, relative, sep } from "node:path";
import { open, TezError } from "../src/api.mjs";
import { siteHtml } from "../runtime/export-site.mjs";

const USAGE = `tez: a fast, verified language for LLMs and the browser

USAGE:
    tez new <dir> [--app]                    start a project: a small program, or a web app
    tez check <file> [--json] [--no-verify]  type-check and verify a program
    tez run <file> [fn] [--no-verify] [--contracts]
                                            run a zero-argument function (default: main)
    tez test <file> [--no-verify] [--contracts]
                                            run the \`test\` items
    tez build <file> [-o out.wasm] [--no-verify]
                                            compile to a WebAssembly (WasmGC) module
    tez fmt <file|dir> [--check | --write]  print / check / rewrite canonical formatting
                                            (a directory: every .tez file below it; needs --check or --write)
    tez site <file> [-o index.html] [--allow=https://origin]... [--no-verify]
                                            write one self-contained web page (program + runtime)
    tez grammar                             print the normative grammar (EBNF)
    tez --version

\`check\`, \`run\`, \`test\`, \`build\` and \`site\` take \`--watch\`: run again whenever a .tez file next to the entry file
(or below it) changes; stop with Ctrl-C.

Programs are verified before they run. \`--no-verify\` skips verification (nothing is then
guaranteed); \`--contracts\` evaluates requires/ensures/refinements while running.
Verification needs an SMT solver: the \`z3\` executable if present, else the bundled WebAssembly Z3.
`;

/** The flags each command accepts; anything else is a usage error (a typo must not be ignored silently). */
const FLAGS = {
  check: ["--json", "--no-verify", "--watch"],
  run: ["--json", "--no-verify", "--contracts", "--watch"],
  test: ["--json", "--no-verify", "--contracts", "--watch"],
  build: ["--json", "--no-verify", "--watch"],
  site: ["--json", "--no-verify", "--watch", "--allow="],
  fmt: ["--check", "--write"],
  new: ["--app"],
};

const argv = process.argv.slice(2).map((a) => (a === "-h" ? "--help" : a));
const flags = new Set(argv.filter((a) => a.startsWith("--")));
const allow = argv.filter((a) => a.startsWith("--allow=")).map((a) => a.slice(8));
const pos = [];
let out = null;
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === "-o") out = argv[++i];
  else if (!argv[i].startsWith("--")) pos.push(argv[i]);
}
const cmd = argv[0]?.startsWith("-") && argv[0] !== "-o" ? argv[0] : (pos[0] ?? "");
const [file, ...rest] = cmd === pos[0] ? pos.slice(1) : [];

/** A message and an exit code, thrown so that `--watch` can carry on after one. */
class Exit extends Error {
  constructor(message, code = 2) {
    super(message);
    this.code = code;
  }
}
function fail(msg, code = 2) {
  throw new Exit(`tez: ${msg}`, code);
}

const SKIP = (name) => name === "node_modules" || name === "target" || name.startsWith(".");

/** Every `.tez` file at or below `dir`, with modification time and size (sorted by path). */
function tezFiles(dir) {
  const found = [];
  const walk = (d, depth) => {
    let names;
    try {
      names = readdirSync(d);
    } catch {
      return;
    }
    for (const name of names) {
      if (SKIP(name)) continue;
      const p = join(d, name);
      let st;
      try {
        st = statSync(p);
      } catch {
        continue;
      }
      if (st.isDirectory()) {
        if (depth < 6) walk(p, depth + 1);
      } else if (name.endsWith(".tez")) found.push(`${p}:${st.mtimeMs}:${st.size}`);
    }
  };
  walk(dir, 0);
  return found.sort().join("\n");
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
  if (flags.has("--help") || cmd === "--help" || cmd === "help") {
    process.stdout.write(USAGE);
    return 0;
  }
  if (cmd === "") {
    console.error(`tez: a command is needed\n\n${USAGE}`);
    return 2;
  }
  if (cmd === "grammar") {
    process.stdout.write(readFileSync(new URL("../grammar.ebnf", import.meta.url), "utf8"));
    return 0;
  }
  if (!Object.keys(FLAGS).includes(cmd)) {
    console.error(`tez: unknown command \`${cmd}\`\n\n${USAGE}`);
    return 2;
  }
  const bad = [...flags].find((f) => !FLAGS[cmd].some((ok) => (ok.endsWith("=") ? f.startsWith(ok) : f === ok)));
  if (bad) fail(`\`${cmd}\` does not take \`${bad}\``);
  if (cmd === "new") return newProject();
  if (!file) fail(`\`${cmd}\` needs a file`);
  if (!existsSync(file)) fail(`cannot read \`${file}\`: no such file`);

  if (flags.has("--watch")) return watch();
  return runOnce();
}

/** Runs the command, then again every time a `.tez` file of the project changes. Never returns. */
async function watch() {
  const dir = dirname(file);
  let seen = tezFiles(dir);
  for (;;) {
    if (process.stdout.isTTY) process.stdout.write("\x1b[2J\x1b[H");
    try {
      await runOnce();
    } catch (e) {
      if (!(e instanceof Exit)) throw e;
      console.error(e.message);
    }
    console.log(`\n[watching ${file}: it runs again when a .tez file changes; Ctrl-C stops]`);
    for (;;) {
      await new Promise((r) => setTimeout(r, 250));
      if (tezFiles(dir) === seen) continue;
      // an editor may write in several steps: wait until the files hold still
      for (;;) {
        await new Promise((r) => setTimeout(r, 100));
        const now = tezFiles(dir);
        if (now === tezFiles(dir)) {
          seen = now;
          break;
        }
      }
      break;
    }
  }
}

/** `tez new <dir> [--app]`: a directory with a `main.tez` that already checks, tests and runs. */
function newProject() {
  if (!file) fail("`new` needs a directory name");
  if (existsSync(file) && !statSync(file).isDirectory()) fail(`\`${file}\` exists and is not a directory`);
  if (existsSync(file) && readdirSync(file).length) fail(`\`${file}\` is not empty; nothing was written`);
  const app = flags.has("--app");
  const text = readFileSync(new URL(`../templates/${app ? "app" : "program"}.tez`, import.meta.url), "utf8");
  mkdirSync(file, { recursive: true });
  writeFileSync(join(file, "main.tez"), text);
  console.log(`created ${join(file, "main.tez")}`);
  console.log(`next:  cd ${file} && tez check main.tez && tez test main.tez`);
  console.log(
    app
      ? "       tez site main.tez -o index.html   (one self-contained page), or open it in the Studio:\n       https://mryesiller.github.io/tez-lang/studio/"
      : "       tez run main.tez",
  );
  return 0;
}

async function runOnce() {
  const tez = await open();
  if (cmd === "fmt") return statSync(file).isDirectory() ? fmtDir(tez) : fmt(tez, file);

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
      writeFileSync(target, siteHtml({ title: basename(file, ".tez"), wasm: b.wasm, runtime: here("tez-runtime.mjs"), app: here("tez-app.mjs"), allow }));
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

  function fmt(t, path) {
    const src = readFileSync(path, "utf8");
    const r = t.format(src);
    if (!r.ok) {
      if (r.comment) console.error(`tez: ${path}:${r.comment.line}:${r.comment.col}: a comment sits inside an expression; move it onto its own line before a statement`);
      else process.stderr.write(JSON.stringify(r.report));
      return 1;
    }
    if (flags.has("--check")) {
      if (r.text === src) return 0;
      console.error(`tez: ${path} is not formatted`);
      return 1;
    }
    if (flags.has("--write")) {
      if (r.text !== src) writeFileSync(path, r.text);
      return 0;
    }
    process.stdout.write(r.text);
    return 0;
  }

  /** `tez fmt <dir> --check|--write`: every `.tez` file below the directory. */
  function fmtDir(t) {
    if (!flags.has("--check") && !flags.has("--write")) fail("`fmt` on a directory needs --check or --write");
    const paths = [];
    const walk = (d, depth) => {
      for (const name of readdirSync(d)) {
        if (SKIP(name)) continue;
        const p = join(d, name);
        if (statSync(p).isDirectory()) {
          if (depth < 6) walk(p, depth + 1);
        } else if (name.endsWith(".tez")) paths.push(p);
      }
    };
    walk(file, 0);
    if (!paths.length) fail(`no .tez files below \`${file}\``);
    let worst = 0;
    for (const p of paths.sort()) worst = Math.max(worst, fmt(t, p));
    return worst;
  }
}

try {
  process.exitCode = await main();
} catch (e) {
  if (!(e instanceof Exit)) throw e;
  console.error(e.message);
  process.exitCode = e.code;
}
