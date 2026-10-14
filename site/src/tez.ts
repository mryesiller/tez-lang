// Everything the site needs from Tez itself: the compiler (Rust → WebAssembly), the runtime and the app host.
import init, * as tez from "../../studio/src/tez-wasm/tez_web.js";
import { load } from "../../runtime/tez-runtime.mjs";
import { isApp, mountApp, type AppHandle } from "../../runtime/tez-app.mjs";

const ready = init();

export type Diagnostic = { code: string; message: string; span: { start: { line: number; col: number } }; expected?: string };

function bytesOf(b64: string): Uint8Array {
  return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
}

export async function check(src: string): Promise<Diagnostic[]> {
  await ready;
  return (JSON.parse(tez.check(src)) as { report: { diagnostics: Diagnostic[] } }).report.diagnostics;
}

type Built = { ok: boolean; wasm?: string; report?: { diagnostics: Diagnostic[] }; error?: string };

export async function build(src: string) {
  await ready;
  const b = JSON.parse(tez.build(src)) as Built;
  if (!b.ok || !b.wasm) return { ok: false as const, diagnostics: b.report?.diagnostics ?? [], error: b.error };
  return { ok: true as const, rt: await load(bytesOf(b.wasm)), size: Math.round(b.wasm.length * 0.75) };
}

/** Runs `main()` compiled to WebAssembly and returns its value, or the diagnostics that stopped it. */
export async function run(src: string): Promise<{ ok: true; value: string; ms: number } | { ok: false; text: string }> {
  const b = await build(src);
  if (!b.ok) return { ok: false, text: formatDiagnostics(b.diagnostics) || b.error || "build failed" };
  const t0 = performance.now();
  try {
    return { ok: true, value: b.rt.run("main"), ms: Math.round((performance.now() - t0) * 10) / 10 };
  } catch (e) {
    return { ok: false, text: e instanceof Error ? e.message : String(e) };
  }
}

export function formatDiagnostics(ds: Diagnostic[]): string {
  return ds
    .map((d) => `${d.code} (line ${d.span.start.line}, col ${d.span.start.col}): ${d.message}${d.expected ? `\n  expected ${d.expected}` : ""}`)
    .join("\n");
}

/** Mounts `src` as an app in `root`; later calls to `update(src)` hot-swap while keeping the session. */
export function appRunner(root: HTMLElement, onStatus: (text: string, ok: boolean) => void) {
  let handle: AppHandle | null = null;
  let seq = 0;
  return async function update(src: string) {
    const mine = ++seq;
    const b = await build(src);
    if (mine !== seq) return;
    if (!b.ok) return onStatus(formatDiagnostics(b.diagnostics) || b.error || "build failed", false);
    if (!isApp(b.rt)) return onStatus("An app needs init(), update(model, msg) and view(model).", false);
    try {
      if (handle) {
        const r = handle.swap(b.rt);
        onStatus(`hot-swapped (${r.replayed} event${r.replayed === 1 ? "" : "s"} replayed), ${b.size} bytes of WebAssembly`, true);
      } else {
        handle = mountApp(b.rt, root, { onError: (e) => onStatus(`stopped: ${e.message}`, false) });
        onStatus(`running as WebAssembly, ${b.size} bytes`, true);
      }
    } catch (e) {
      onStatus(`${e instanceof Error ? e.message : e}`, false);
    }
  };
}

// ---------------------------------------------------------------- syntax highlighting

const KEYWORDS = new Set(["fn", "type", "const", "test", "let", "match", "if", "else", "where", "with", "requires", "ensures", "decreases", "true", "false", "it", "result", "extern"]);

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

export function highlight(src: string): string {
  const re = /(\/\/[^\n]*)|("(?:[^"\\\n]|\\.)*")|(\b\d[\d_]*(?:\.\d+)?\b)|([A-Za-z_][A-Za-z0-9_]*)|([\s\S])/g;
  let out = "";
  let prev = "";
  for (const m of src.matchAll(re)) {
    const [t, comment, str, num, word] = m;
    if (comment) out += `<span class="c">${esc(comment)}</span>`;
    else if (str) out += `<span class="s">${esc(str)}</span>`;
    else if (num) out += `<span class="n">${num}</span>`;
    else if (word) {
      if (KEYWORDS.has(word)) out += `<span class="k">${word}</span>`;
      else if (/^[A-Z]/.test(word)) out += `<span class="t">${word}</span>`;
      else if (prev === "fn") out += `<span class="f">${word}</span>`;
      else out += word;
      prev = word;
      continue;
    } else out += esc(t);
    if (t.trim()) prev = t;
  }
  return out;
}
