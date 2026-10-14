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

type Pending = { resolve: (json: string) => void };

export class Compiler {
  private worker = new Worker(new URL("./compiler.worker.ts", import.meta.url), { type: "module" });
  private next = 1;
  private pending = new Map<number, Pending>();

  constructor() {
    this.worker.onmessage = (ev: MessageEvent<{ id: number; json: string }>) => {
      const p = this.pending.get(ev.data.id);
      if (p) {
        this.pending.delete(ev.data.id);
        p.resolve(ev.data.json);
      }
    };
  }

  private call<T>(op: string, src: string, extra: object = {}): Promise<T> {
    const id = this.next++;
    return new Promise<T>((resolve) => {
      this.pending.set(id, { resolve: (json) => resolve(JSON.parse(json) as T) });
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
