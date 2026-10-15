// Promise-based client for the compiler worker.

export type Pos = { line: number; col: number };
export type Span = { start: Pos; end: Pos };
export type Edit = { span: Span; replace: string };
export type Fix = { title: string; edits: Edit[] };
export type Diagnostic = {
  code: string;
  phase: string;
  message: string;
  span: Span;
  expected?: string;
  counterexample?: Record<string, string>;
  fixes?: Fix[];
};
export type Report = { version: number; status: "ok" | "error"; diagnostics: Diagnostic[] };

export type CheckResult = { report: Report; verified: boolean };
export type FormatResult = { ok: boolean; text?: string; report?: Report; comment?: Pos };
export type RunResult = { ok: boolean; value?: string; report?: Report; crash?: string; ms?: number; bytes?: number };
export type TestResult = {
  ok: boolean;
  results?: { name: string; outcome: "pass" | "fail" | "error"; message?: string; code?: string }[];
  report?: Report;
};

type Pending = { resolve: (json: string) => void; timer?: ReturnType<typeof setTimeout> };

/** Running a program is bounded: past this the worker is killed and replaced (the language has no loops, but
 *  an unverified program can still recurse for ever or take exponential time). */
export const RUN_TIMEOUT_MS = 10_000;

export type VerifyMissing = { id: string; query: string; wantsValues: boolean };
export type VerifyStep = { done: false; missing: VerifyMissing[] } | { done: true; verified: boolean; report: Report };
export type VerifyResult = { verified: boolean; report: Report; rounds: number; queries: number };

type Z3Api = {
  mk_config(): number;
  mk_context(cfg: number): number;
  del_config(cfg: number): void;
  del_context(ctx: number): void;
  eval_smtlib2_string(ctx: number, s: string): Promise<string>;
};

/**
 * Z3 compiled to WebAssembly, started on first use because it is 33 MB. It runs on the page, not in a worker: Z3's
 * build is multithreaded and starts its own thread workers, which only the main thread may do reliably; its calls
 * are asynchronous, so the page stays responsive. Needs cross-origin isolation (SharedArrayBuffer).
 */
class Z3Client {
  private api: Promise<Z3Api> | null = null;

  private async load(): Promise<Z3Api> {
    if (!self.crossOriginIsolated) {
      throw new Error("verification needs a cross-origin isolated page (COOP/COEP headers) for Z3's threads");
    }
    const base = new URL(`${import.meta.env.BASE_URL}z3/`, document.baseURI).href;
    // Z3's loader is a classic script that defines a global `initZ3`, and its thread workers re-load that same script
    await new Promise<void>((resolve, reject) => {
      const tag = document.createElement("script");
      tag.src = `${base}z3-built.js`;
      tag.onload = () => resolve();
      tag.onerror = () => reject(new Error("could not load Z3 (z3-built.js)"));
      document.head.append(tag);
    });
    (window as unknown as { global: unknown }).global = window;
    const { init } = await import("z3-solver");
    const z3 = await init({ locateFile: (f: string) => `${base}${f}` } as never);
    return z3.Z3 as unknown as Z3Api;
  }

  async solve(query: string): Promise<string> {
    this.api ??= this.load();
    const z3 = await this.api;
    const cfg = z3.mk_config();
    const ctx = z3.mk_context(cfg);
    z3.del_config(cfg);
    try {
      return await z3.eval_smtlib2_string(ctx, query);
    } finally {
      z3.del_context(ctx);
    }
  }
}

export class Compiler {
  private z3 = new Z3Client();
  private worker = this.spawn();
  private next = 1;
  private pending = new Map<number, Pending>();

  private spawn() {
    const w = new Worker(new URL("./compiler.worker.ts", import.meta.url), { type: "module" });
    w.onmessage = (ev: MessageEvent<{ id: number; json: string }>) => {
      const p = this.pending.get(ev.data.id);
      if (p) {
        clearTimeout(p.timer);
        this.pending.delete(ev.data.id);
        p.resolve(ev.data.json);
      }
    };
    return w;
  }

  /** Kills a runaway worker, answers its request with a resource error, and starts a fresh worker. */
  private timedOut(id: number, op: string) {
    const p = this.pending.get(id);
    if (!p) return;
    this.pending.delete(id);
    const message = `the program ran for more than ${RUN_TIMEOUT_MS / 1000} s and was stopped`;
    const report = {
      version: 1,
      status: "error",
      diagnostics: [{ code: "E_RESOURCE", phase: "runtime", message, span: { start: { line: 1, col: 1 }, end: { line: 1, col: 1 } } }],
    };
    p.resolve(JSON.stringify(op === "test" ? { ok: false, report } : { ok: false, report }));
    // every other request waiting on this worker is lost with it: tell them to retry
    for (const [other, q] of this.pending) {
      clearTimeout(q.timer);
      this.pending.delete(other);
      q.resolve(JSON.stringify({ ok: false, crash: "the compiler worker was restarted; try again", report: { version: 1, status: "error", diagnostics: [] }, report_restarted: true }));
    }
    this.worker.terminate();
    this.worker = this.spawn();
  }

  private call<T>(op: string, src: string, extra: object = {}): Promise<T> {
    const id = this.next++;
    return new Promise<T>((resolve) => {
      const running = op === "run" || op === "test" || op === "interpret";
      const timer = running ? setTimeout(() => this.timedOut(id, op), RUN_TIMEOUT_MS) : undefined;
      this.pending.set(id, { resolve: (json) => resolve(JSON.parse(json) as T), timer });
      this.worker.postMessage({ id, op, src, ...extra });
    });
  }

  check(src: string) {
    return this.call<CheckResult>("check", src);
  }
  format(src: string) {
    return this.call<FormatResult>("format", src);
  }
  run(src: string, name = "main", contracts = false) {
    return this.call<RunResult>("run", src, { name, contracts });
  }
  build(src: string) {
    return this.call<{ ok: boolean; wasm?: string; report?: Report; error?: string }>("build", src);
  }
  /**
   * Proves the program's contracts in the browser. The verifier works in rounds (see `verify_step` in tez_web):
   * it reports which solver queries it still needs, Z3-WebAssembly answers them, and it runs again.
   */
  async verify(src: string, onProgress: (text: string) => void = () => {}): Promise<VerifyResult> {
    const answers: Record<string, string> = {};
    let queries = 0;
    for (let round = 1; round <= 80; round++) {
      const step = await this.call<VerifyStep>("verify_step", src, { answers: JSON.stringify(answers) });
      if (step.done) return { verified: step.verified, report: step.report, rounds: round, queries };
      onProgress(`round ${round}: solving ${step.missing.length} quer${step.missing.length === 1 ? "y" : "ies"} (${queries} so far)`);
      for (const m of step.missing) {
        answers[m.id] = await this.z3.solve(m.query);
        queries++;
      }
    }
    throw new Error("verification did not converge in 80 rounds");
  }

  test(src: string, contracts = false) {
    return this.call<TestResult>("test", src, { contracts });
  }
}
