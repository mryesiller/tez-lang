/// <reference lib="webworker" />
// The Tez compiler (Rust, built to WebAssembly) running off the main thread.
import init, * as tez from "./tez-wasm/tez_web.js";

type Request = { id: number; op: "check" | "format" | "run" | "test"; src: string; name?: string; contracts?: boolean };

const ready = init();

self.onmessage = async (ev: MessageEvent<Request>) => {
  const { id, op, src, name, contracts } = ev.data;
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
