import Editor, { type OnMount } from "@monaco-editor/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Compiler, type Diagnostic, type Fix } from "./compiler";
import { load } from "../../runtime/tez-runtime.mjs";
import { isApp, mountApp, type AppHandle } from "../../runtime/tez-app.mjs";
import "./monaco";

const exampleFiles = import.meta.glob("../../spec/examples/*.tez", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const examples = Object.entries(exampleFiles)
  .map(([path, source]) => ({ name: path.split("/").pop() ?? path, source }))
  .filter((e) => !e.name.endsWith(".reject.tez"))
  .sort((a, b) => a.name.localeCompare(b.name));

const rejects = Object.entries(exampleFiles)
  .map(([path, source]) => ({ name: path.split("/").pop() ?? path, source }))
  .filter((e) => e.name.endsWith(".reject.tez"))
  .sort((a, b) => a.name.localeCompare(b.name));

const FALLBACK = `fn main() -> Int = 6 * 7\n`;

type Monaco = Parameters<OnMount>[1];
type EditorInstance = Parameters<OnMount>[0];

export default function App() {
  const compiler = useMemo(() => new Compiler(), []);
  const [source, setSource] = useState(examples[0]?.source ?? FALLBACK);
  const [diags, setDiags] = useState<Diagnostic[]>([]);
  const [output, setOutput] = useState<{ kind: "idle" | "ok" | "error"; text: string }>({
    kind: "idle",
    text: "Press Run to evaluate main().",
  });
  const [busy, setBusy] = useState(false);
  const editorRef = useRef<EditorInstance | null>(null);
  const monacoRef = useRef<Monaco | null>(null);
  const seq = useRef(0);
  const appRoot = useRef<HTMLDivElement | null>(null);
  const app = useRef<AppHandle | null>(null);
  const [appOn, setAppOn] = useState(false);

  const publish = useCallback((ds: Diagnostic[]) => {
    setDiags(ds);
    const monaco = monacoRef.current;
    const model = editorRef.current?.getModel();
    if (!monaco || !model) return;
    monaco.editor.setModelMarkers(
      model,
      "tez",
      ds.map((d) => ({
        severity: monaco.MarkerSeverity.Error,
        message: `${d.code}: ${d.message}${d.expected ? `\nexpected: ${d.expected}` : ""}`,
        startLineNumber: d.span.start.line,
        startColumn: d.span.start.col,
        endLineNumber: d.span.end.line,
        endColumn: Math.max(d.span.end.col, d.span.start.col + 1),
      })),
    );
  }, []);

  // live diagnostics: debounce, and ignore answers to stale requests
  useEffect(() => {
    const mine = ++seq.current;
    const t = setTimeout(async () => {
      const r = await compiler.check(source);
      if (mine === seq.current) publish(r.report.diagnostics);
    }, 150);
    return () => clearTimeout(t);
  }, [source, compiler, publish]);

  const onMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    if (import.meta.env.DEV) (window as unknown as { __tezEditor: unknown }).__tezEditor = editor;
    monacoRef.current = monaco;
  };

  const showReport = (r: { report?: { diagnostics: Diagnostic[] }; crash?: string }) =>
    r.crash
      ? `compiler crashed: ${r.crash}`
      : (r.report?.diagnostics ?? []).map((d) => `${d.code}: ${d.message}`).join("\n");

  const run = async () => {
    setBusy(true);
    const r = await compiler.run(source, "main", false);
    setOutput(
      r.ok
        ? { kind: "ok", text: `${r.value ?? ""}\n\n(compiled to WebAssembly: ${Math.round(r.bytes ?? 0)} bytes, ran in ${r.ms ?? "?"} ms)` }
        : { kind: "error", text: showReport(r) },
    );
    setBusy(false);
  };

  const runApp = async () => {
    setBusy(true);
    app.current?.stop();
    app.current = null;
    const built = await compiler.build(source);
    if (!built.ok || !built.wasm) {
      setOutput({ kind: "error", text: showReport(built) });
      setBusy(false);
      return;
    }
    try {
      const bin = atob(built.wasm);
      const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
      const rt = await load(bytes);
      if (!isApp(rt) || !appRoot.current) {
        setOutput({ kind: "error", text: "An app needs `fn init() -> Model`, `fn update(m: Model, msg: Msg) -> Model` and `fn view(m: Model) -> Html[Msg]`." });
      } else {
        setAppOn(true);
        app.current = mountApp(rt, appRoot.current, {
          onError: (e) => setOutput({ kind: "error", text: `app stopped: ${e.message}` }),
        });
        setOutput({ kind: "ok", text: "App running. Each click calls update(model, msg), then view(model); only changed DOM is touched." });
      }
    } catch (e) {
      setOutput({ kind: "error", text: String(e instanceof Error ? e.message : e) });
    }
    setBusy(false);
  };

  useEffect(() => () => app.current?.stop(), []);

  // hot-swap: a valid edit while the app runs replaces the program but keeps the user's session
  useEffect(() => {
    if (!app.current) return;
    const t = setTimeout(async () => {
      const built = await compiler.build(source);
      if (!built.ok || !built.wasm || !app.current) return;
      try {
        const rt = await load(Uint8Array.from(atob(built.wasm), (c) => c.charCodeAt(0)));
        if (!isApp(rt)) return;
        const r = app.current.swap(rt);
        setOutput({ kind: "ok", text: `Hot-swapped: replayed ${r.replayed} event(s)${r.dropped ? `, dropped ${r.dropped} that no longer apply` : ""}.` });
      } catch (e) {
        setOutput({ kind: "error", text: `hot-swap failed, the previous version keeps running: ${e instanceof Error ? e.message : e}` });
      }
    }, 400);
    return () => clearTimeout(t);
  }, [source, compiler]);

  const runTests = async () => {
    setBusy(true);
    const r = await compiler.test(source, false);
    if (!r.ok || !r.results) {
      setOutput({ kind: "error", text: showReport(r) });
    } else if (r.results.length === 0) {
      setOutput({ kind: "idle", text: "No `test` items in this program." });
    } else {
      const lines = r.results.map((t) => `${t.outcome === "pass" ? "ok     " : "FAILED "} ${t.name}${t.message ? `: ${t.message}` : ""}`);
      const failed = r.results.filter((t) => t.outcome !== "pass").length;
      setOutput({ kind: failed ? "error" : "ok", text: `${lines.join("\n")}\n${r.results.length - failed} passed, ${failed} failed` });
    }
    setBusy(false);
  };

  const format = async () => {
    const r = await compiler.format(source);
    if (r.ok && r.text !== undefined) setSource(r.text);
    else if (r.comment) setOutput({ kind: "error", text: `A comment at ${r.comment.line}:${r.comment.col} sits inside an expression; move it onto its own line.` });
  };

  const applyFix = (fix: Fix) => {
    const editor = editorRef.current;
    const monaco = monacoRef.current;
    if (!editor || !monaco) return;
    editor.executeEdits(
      "tez-fix",
      fix.edits.map((e) => ({
        range: new monaco.Range(e.span.start.line, e.span.start.col, e.span.end.line, e.span.end.col),
        text: e.replace,
      })),
    );
  };

  return (
    <div className="studio">
      <header className="bar">
        <strong>Tez Studio</strong>
        <select
          aria-label="Load an example"
          onChange={(e) => {
            const all = [...examples, ...rejects];
            const found = all.find((x) => x.name === e.target.value);
            if (found) setSource(found.source);
          }}
          defaultValue=""
        >
          <option value="" disabled>
            Examples…
          </option>
          <optgroup label="Programs">
            {examples.map((x) => (
              <option key={x.name}>{x.name}</option>
            ))}
          </optgroup>
          <optgroup label="Rejected on purpose">
            {rejects.map((x) => (
              <option key={x.name}>{x.name}</option>
            ))}
          </optgroup>
        </select>
        <button onClick={format}>Format</button>
        <button onClick={run} disabled={busy || diags.length > 0}>
          Run
        </button>
        <button onClick={runApp} disabled={busy || diags.length > 0} title="Run init/update/view as an app">
          App
        </button>
        <button onClick={runTests} disabled={busy || diags.length > 0}>
          Test
        </button>
        <span className="muted spacer">
          Types, patterns and names are checked here. Contract proofs (SMT) run in the <code>tez</code> CLI for now.
        </span>
      </header>
      <main className="panes">
        <section className="pane" aria-label="Source editor">
          <Editor
            language="tez"
            theme="vs-dark"
            value={source}
            onChange={(value) => setSource(value ?? "")}
            onMount={onMount}
            options={{ minimap: { enabled: false }, fontSize: 14, tabSize: 2, automaticLayout: true }}
          />
        </section>
        <section className="pane side" aria-label="Diagnostics and output">
          <h2>Diagnostics {diags.length === 0 ? <span className="ok">none</span> : <span className="bad">{diags.length}</span>}</h2>
          {diags.length === 0 && <p className="muted">No problems.</p>}
          <ul className="diags">
            {diags.map((d, i) => (
              <li key={i}>
                <div>
                  <code className="code">{d.code}</code> {d.message}
                </div>
                <div className="muted">
                  line {d.span.start.line}, column {d.span.start.col}
                  {d.expected ? ` · expected ${d.expected}` : ""}
                </div>
                {(d.fixes ?? []).map((f, j) => (
                  <button key={j} className="fix" onClick={() => applyFix(f)}>
                    Fix: {f.title}
                  </button>
                ))}
              </li>
            ))}
          </ul>
          <h2 hidden={!appOn}>App</h2>
          <div className="app" ref={appRoot} hidden={!appOn} />
          <h2>Output</h2>
          <pre className={`out ${output.kind}`}>{output.text}</pre>
        </section>
      </main>
    </div>
  );
}
