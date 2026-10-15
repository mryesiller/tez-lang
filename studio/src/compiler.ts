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

export class Compiler {
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
  test(src: string, contracts = false) {
    return this.call<TestResult>("test", src, { contracts });
  }
}
