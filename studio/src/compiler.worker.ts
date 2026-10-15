/// <reference lib="webworker" />
// The Tez compiler (Rust, built to WebAssembly) running off the main thread.
import init, * as tez from "./tez-wasm/tez_web.js";
import { load, TezError } from "../../runtime/tez-runtime.mjs";

type Request = { id: number; op: "check" | "format" | "run" | "interpret" | "test" | "build" | "verify_step"; src: string; name?: string; contracts?: boolean; answers?: string };

const ready = init();

function fromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Compiles to WebAssembly (GC) and runs the function there; the result mimics `tez.run`. */
async function runCompiled(src: string, name: string): Promise<string> {
  const built = JSON.parse(tez.build(src)) as { ok: boolean; wasm?: string; report?: unknown; error?: string };
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
  await ready;
  let json: string;
  try {
    switch (op) {
      case "check":
        json = tez.check(src);
        break;
      case "format":
        json = tez.format(src);
        break;
      case "run":
        json = contracts ? tez.run(src, name ?? "main", true) : await runCompiled(src, name ?? "main");
        break;
      case "verify_step":
        json = tez.verify_step(src, answers ?? "{}");
        break;
      case "build":
        json = tez.build(src);
        break;
      case "interpret":
        json = tez.run(src, name ?? "main", contracts ?? false);
        break;
      default:
        json = tez.test(src, contracts ?? false);
    }
  } catch (e) {
    json = JSON.stringify({ ok: false, crash: String(e) });
  }
  (self as DedicatedWorkerGlobalScope).postMessage({ id, json });
};
