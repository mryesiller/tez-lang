/// <reference lib="webworker" />
// The Tez compiler (Rust, built to WebAssembly) running off the main thread.
import init, * as tez from "./tez-wasm/tez_web.js";
import { load, TezError } from "../../runtime/tez-runtime.mjs";

type Request = {
  id: number;
  op: "check" | "format" | "run" | "interpret" | "test" | "build" | "verify_step";
  /** the project: `{"main.tez": "...", "lib/x.tez": "..."}` as JSON; for `format`, the text of one file */
  src: string;
  entry?: string;
  name?: string;
  contracts?: boolean;
  answers?: string;
};

const ready = init();

function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Compiles to WebAssembly (GC) and runs the function there; the result mimics `tez.run`. */
async function runCompiled(entry: string, src: string, name: string): Promise<string> {
  const built = JSON.parse(tez.build_project(entry, src, "")) as { ok: boolean; wasm?: string; report?: unknown; error?: string; human?: string };
  if (!built.ok || !built.wasm) return JSON.stringify({ ok: false, report: built.report, crash: built.error });
  const t0 = performance.now();
  try {
    const rt = await load(fromBase64(built.wasm));
    const value = rt.run(name);
    return JSON.stringify({ ok: true, value, ms: Math.round((performance.now() - t0) * 10) / 10, bytes: built.wasm.length * 0.75 });
  } catch (e) {
    const code = e instanceof TezError ? e.code : "E_INTERNAL";
    const message = e instanceof Error ? e.message : String(e);
    return JSON.stringify({
      ok: false,
      report: {
        version: 1,
        status: "error",
        diagnostics: [{ code, phase: "runtime", message, span: { start: { line: 1, col: 1 }, end: { line: 1, col: 1 } } }],
      },
    });
  }
}

self.onmessage = async (ev: MessageEvent<Request>) => {
  const { id, op, src, name, contracts, answers } = ev.data;
  const entry = ev.data.entry ?? "main.tez";
  await ready;
  let json: string;
  try {
    switch (op) {
      case "check":
        json = tez.check_project(entry, src, "");
        break;
      case "format":
        json = tez.format(src);
        break;
      case "run":
        json = contracts ? tez.run_project(entry, src, name ?? "main", true, "") : await runCompiled(entry, src, name ?? "main");
        break;
      case "verify_step":
        json = tez.verify_project_step(entry, src, answers ?? "{}", "");
        break;
      case "build":
        json = tez.build_project(entry, src, "");
        break;
      case "interpret":
        json = tez.run_project(entry, src, name ?? "main", contracts ?? false, "");
        break;
      default:
        json = tez.test_project(entry, src, contracts ?? false, "");
    }
  } catch (e) {
    json = JSON.stringify({ ok: false, crash: String(e) });
  }
  (self as DedicatedWorkerGlobalScope).postMessage({ id, json });
};
