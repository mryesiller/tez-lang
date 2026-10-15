// The Tez compiler as a library: `import { open } from "tez-lang"`.
// Everything works on a map of project files `{ "main.tez": "...", "lib/x.tez": "..." }`; `use "x.tez"` lines are
// resolved against that map only.
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { load as loadRuntime, TezError } from "../runtime/tez-runtime.mjs";
import init, * as tez from "../wasm/tez_web.js";
import { findSolver } from "./z3.mjs";

export { TezError };

/** Instantiates the compiler once. */
export async function open() {
  await init({ module_or_path: readFileSync(fileURLToPath(new URL("../wasm/tez_web_bg.wasm", import.meta.url))) });
  const json = (s) => JSON.parse(s);
  const files = (f) => JSON.stringify(f);
  return {
    version: () => tez.version(),
    check: (entry, f, dir = "") => json(tez.check_project(entry, files(f), dir)),
    format: (src) => json(tez.format(src)),
    build: (entry, f, dir = "") => json(tez.build_project(entry, files(f), dir)),
    run: (entry, f, name = "main", contracts = false, dir = "") => json(tez.run_project(entry, files(f), name, contracts, dir)),
    test: (entry, f, contracts = false, dir = "") => json(tez.test_project(entry, files(f), contracts, dir)),
    /** Type-checks and proves the program. Resolves `{verified, report, human, solver}`; rejects when no solver exists. */
    async verify(entry, f, dir = "", { onRound } = {}) {
      const solver = await findSolver();
      if (!solver) throw new Error("no SMT solver: install `z3` (https://github.com/Z3Prover/z3) or run `npm i z3-solver`, or use --no-verify");
      const answers = {};
      try {
        for (let round = 1; round <= 200; round++) {
          const step = json(tez.verify_project_step(entry, files(f), JSON.stringify(answers), dir));
          if (step.done) return { ...step, solver: solver.name, rounds: round };
          onRound?.(round, step.missing.length);
          const solved = await Promise.all(step.missing.map((m) => solver.solve(m.query)));
          step.missing.forEach((m, i) => (answers[m.id] = solved[i]));
        }
        throw new Error("verification did not converge");
      } finally {
        solver.close?.();
      }
    },
    /** Runs a built module with the Tez runtime. */
    load: (wasmBase64) => loadRuntime(Buffer.from(wasmBase64, "base64")),
  };
}
