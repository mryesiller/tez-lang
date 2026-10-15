// Solvers for the verifier's queries. The compiler (WebAssembly) works in rounds: it says which SMT-LIB scripts it
// still needs answered, a solver answers them, and it runs again (see `verify_project_step` in crates/tez_web).
// Order: the `z3` executable (fast; `TEZ_Z3` overrides its path), else Z3 compiled to WebAssembly (`z3-solver`,
// an optional dependency). No solver means nothing can be proved, so callers must treat that as "not verified".
import { spawn } from "node:child_process";

function viaProcess(exe) {
  return (query) =>
    new Promise((resolve, reject) => {
      const p = spawn(exe, ["-in", "-smt2"], { stdio: ["pipe", "pipe", "ignore"] });
      let out = "";
      p.stdout.on("data", (d) => (out += d));
      p.on("error", reject);
      p.on("close", () => resolve(out));
      p.stdin.on("error", () => {});
      p.stdin.end(query);
    });
}

function canRun(exe) {
  return new Promise((resolve) => {
    const p = spawn(exe, ["--version"], { stdio: "ignore" });
    p.on("error", () => resolve(false));
    p.on("close", (code) => resolve(code === 0));
  });
}

async function viaWasm() {
  let init;
  try {
    ({ init } = await import("z3-solver"));
  } catch {
    return null;
  }
  const z3 = await init();
  const api = z3.Z3;
  let queue = Promise.resolve(); // Z3's WebAssembly build runs one asynchronous call at a time
  return {
    name: "z3-solver (WebAssembly)",
    solve: (query) => (queue = queue.then(() => one(query), () => one(query))),
    close: () => z3.em?.PThread?.terminateAllThreads?.(),
  };
  async function one(query) {
      const cfg = api.mk_config();
      const ctx = api.mk_context(cfg);
      api.del_config(cfg);
      try {
        return await api.eval_smtlib2_string(ctx, query);
      } finally {
        api.del_context(ctx);
      }
  }
}

/** Finds a solver: `{name, solve(query) -> Promise<string>, close?}`, or `null` when there is none. */
export async function findSolver() {
  const exe = process.env.TEZ_Z3 || "z3";
  if (await canRun(exe)) return { name: exe, solve: viaProcess(exe) };
  return viaWasm();
}
