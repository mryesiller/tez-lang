import Editor, { type OnMount } from "@monaco-editor/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Compiler, type Diagnostic, type Files, type Fix } from "./compiler";
import { load } from "../../runtime/tez-runtime.mjs";
import card from "../../spec/card.md?raw";
import runtimeSrc from "../../runtime/tez-runtime.mjs?raw";
import appSrc from "../../runtime/tez-app.mjs?raw";
import styleSrc from "./styles.css?raw";
import { siteHtml } from "../../runtime/export-site.mjs";
import { solve } from "../../tools/agent/loop.mjs";
import { inBrowserModel, webGpuAvailable } from "./localModel";
import { openAICompatible } from "../../tools/agent/providers.mjs";
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

// multi-file example projects: spec/examples/projects/<name>/**/*.tez (the entry is main.tez)
const projectFiles = import.meta.glob("../../spec/examples/projects/**/*.tez", {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

const projects: { name: string; files: Files }[] = Object.entries(
  Object.entries(projectFiles).reduce<Record<string, Files>>((acc, [path, text]) => {
    const rest = path.split("/projects/")[1] ?? "";
    const [name, ...file] = rest.split("/");
    if (name && file.length) (acc[name] ??= {})[file.join("/")] = text;
    return acc;
  }, {}),
)
  .map(([name, files]) => ({ name, files }))
  .sort((a, b) => a.name.localeCompare(b.name));

const ENTRY = "main.tez";
const FALLBACK = `fn main() -> Int = 6 * 7\n`;

/** The project from the last visit (kept in this browser only), else the first example. */
function initialProject(): Files {
  try {
    const saved = JSON.parse(localStorage.getItem("tez-project") ?? "null") as Files | null;
    if (saved && typeof saved[ENTRY] === "string") return saved;
  } catch {
    /* no storage: start from the example */
  }
  return { [ENTRY]: examples[0]?.source ?? FALLBACK };
}

/** A new file name is a project-relative `.tez` path: no leading slash, no `..`, no backslash. */
function validName(name: string, existing: Files): string | null {
  if (!/^[A-Za-z0-9_][A-Za-z0-9_./-]*\.tez$/.test(name) || name.includes("..") || name.includes("//")) {
    return "use a relative path ending in .tez, e.g. util/money.tez";
  }
  return name in existing ? "that file exists" : null;
}

type Monaco = Parameters<OnMount>[1];
type EditorInstance = Parameters<OnMount>[0];

export default function App() {
  const compiler = useMemo(() => new Compiler(), []);
  const [files, setFiles] = useState<Files>(initialProject);
  const [active, setActive] = useState(ENTRY);
  const source = files[active] ?? "";
  const setSource = useCallback((text: string) => setFiles((f) => ({ ...f, [active]: text })), [active]);
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
  const [ai, setAi] = useState(() => {
    try {
      return JSON.parse(localStorage.getItem("tez-ai") ?? "{}") as { baseUrl?: string; model?: string; apiKey?: string };
    } catch {
      return {};
    }
  });
  const [aiTask, setAiTask] = useState("");
  const [aiLocal, setAiLocal] = useState(false);
  const [aiLog, setAiLog] = useState("");

  const publish = useCallback((ds: Diagnostic[]) => setDiags(ds), []);

  // squiggles for the file on screen (diagnostics of other files are listed on the right)
  useEffect(() => {
    const monaco = monacoRef.current;
    const model = editorRef.current?.getModel();
    if (!monaco || !model) return;
    monaco.editor.setModelMarkers(
      model,
      "tez",
      diags
        .filter((d) => (d.file ?? ENTRY) === active)
        .map((d) => ({
        severity: monaco.MarkerSeverity.Error,
        message: `${d.code}: ${d.message}${d.expected ? `\nexpected: ${d.expected}` : ""}`,
        startLineNumber: d.span.start.line,
        startColumn: d.span.start.col,
        endLineNumber: d.span.end.line,
        endColumn: Math.max(d.span.end.col, d.span.start.col + 1),
      })),
    );
  }, [diags, active, files]);

  // live diagnostics: debounce, and ignore answers to stale requests
  useEffect(() => {
    const mine = ++seq.current;
    const t = setTimeout(async () => {
      const r = await compiler.check(files);
      if (mine === seq.current) publish(r.report.diagnostics);
    }, 150);
    return () => clearTimeout(t);
  }, [files, compiler, publish]);

  // remember the project in this browser
  useEffect(() => {
    const t = setTimeout(() => {
      try {
        localStorage.setItem("tez-project", JSON.stringify(files));
      } catch {
        /* private mode: the project just is not remembered */
      }
    }, 400);
    return () => clearTimeout(t);
  }, [files]);

  const onMount: OnMount = (editor, monaco) => {
    editorRef.current = editor;
    if (import.meta.env.DEV) (window as unknown as { __tezEditor: unknown }).__tezEditor = editor;
    monacoRef.current = monaco;
  };

  const showReport = (r: { report?: { diagnostics: Diagnostic[] }; crash?: string }) =>
    r.crash
      ? `compiler crashed: ${r.crash}`
      : (r.report?.diagnostics ?? []).map((d) => `${d.file ? `${d.file}:` : ""}${d.span.start.line}:${d.span.start.col} ${d.code}: ${d.message}`).join("\n");

  const run = async () => {
    setBusy(true);
    const r = await compiler.run(files, "main", false);
    setOutput(
      r.ok
        ? { kind: "ok", text: `${r.value ?? ""}\n\n(compiled to WebAssembly: ${Math.round(r.bytes ?? 0)} bytes, ran in ${r.ms ?? "?"} ms)` }
        : { kind: "error", text: showReport(r) },
    );
    setBusy(false);
  };

  const runApp = async (project: Files = files) => {
    setBusy(true);
    app.current?.stop();
    app.current = null;
    const built = await compiler.build(project);
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
          storage: (() => { try { return window.localStorage; } catch { return null; } })(),
          storageKey: "tez-app:studio",
          fetch: window.fetch.bind(window),
          allow: [location.origin],
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
      const built = await compiler.build(files);
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
  }, [files, compiler]);

  const askAi = async () => {
    if ((!aiLocal && (!ai.baseUrl || !ai.model)) || !aiTask.trim()) {
      setAiLog(aiLocal ? "Write a task." : "Fill in the server URL, the model and a task.");
      return;
    }
    try {
      localStorage.setItem("tez-ai", JSON.stringify(ai));
    } catch {
      /* private mode: the settings just are not remembered */
    }
    setBusy(true);
    setAiLog("asking the model…");
    try {
      const r = await solve({
        task: aiTask,
        card,
        generate: aiLocal ? inBrowserModel((t) => setAiLog(t)) : openAICompatible({ baseUrl: ai.baseUrl ?? "", model: ai.model ?? "", apiKey: ai.apiKey }),
        // a program is accepted only when it is proved, not just type-correct; without a solver, type errors only
        check: async (code) => {
          const project = { [ENTRY]: code };
          const typed = (await compiler.check(project)).report.diagnostics;
          if (typed.length) return typed;
          try {
            setAiLog((l) => `${l}\n  proving…`);
            return (await compiler.verify(project)).report.diagnostics;
          } catch {
            return typed;
          }
        },
        onTurn: (t, i) => setAiLog((l) => `${l}\nturn ${i + 1}: ${t.diagnostics.length ? `${t.diagnostics.length} problem(s), first ${t.diagnostics[0]?.code}` : "accepted"}`),
      });
      setFiles({ [ENTRY]: r.code });
      setActive(ENTRY);
      if (r.ok && /\bfn view\(/.test(r.code)) {
        setAiLog((l) => `${l}\nAccepted and proved. Starting the app…`);
        await runApp({ [ENTRY]: r.code });
      }
      setAiLog((l) => `${l}\n${r.ok ? "Done: the program is accepted." : "Stopped after the last turn; the program still has problems."}`);
    } catch (e) {
      setAiLog(`failed: ${e instanceof Error ? e.message : String(e)}`);
    }
    setBusy(false);
  };

  const verify = async () => {
    setBusy(true);
    setOutput({ kind: "idle", text: "Loading Z3 (WebAssembly, 33 MB the first time)…" });
    try {
      const t0 = performance.now();
      const r = await compiler.verify(files, (text) => setOutput({ kind: "idle", text }));
      publish(r.report.diagnostics);
      const secs = ((performance.now() - t0) / 1000).toFixed(1);
      setOutput(
        r.verified
          ? { kind: "ok", text: `Verified: every contract, index, division and termination argument is proved (${r.queries} solver queries, ${r.rounds} rounds, ${secs} s).` }
          : { kind: "error", text: `Not verified: ${r.report.diagnostics.length} problem(s), see Diagnostics (${secs} s).` },
      );
    } catch (e) {
      setOutput({ kind: "error", text: e instanceof Error ? e.message : String(e) });
    }
    setBusy(false);
  };

  const runTests = async () => {
    setBusy(true);
    const r = await compiler.test(files, false);
    if (!r.ok || !r.results) {
      setOutput({ kind: "error", text: showReport(r) });
    } else if (r.results.length === 0) {
      setOutput({ kind: "idle", text: "No `test` items in this program." });
    } else {
      const lines = r.results.map((t) => `${t.outcome === "pass" ? "ok     " : "FAILED "} ${t.name}${t.message ? `: ${t.message}` : ""}${t.at ? `  (${t.at})` : ""}`);
      const failed = r.results.filter((t) => t.outcome !== "pass").length;
      setOutput({ kind: failed ? "error" : "ok", text: `${lines.join("\n")}\n${r.results.length - failed} passed, ${failed} failed` });
    }
    setBusy(false);
  };

  /** Downloads the program as one self-contained web page (module, runtime and app host inlined). */
  const publish_ = async () => {
    setBusy(true);
    const built = await compiler.build(files);
    if (!built.ok || !built.wasm) {
      setOutput({ kind: "error", text: showReport(built) });
      setBusy(false);
      return;
    }
    const css = styleSrc
      .split("\n")
      .filter((l) => l.startsWith(".app"))
      .join("\n");
    const html = siteHtml({ title: "Tez site", wasm: built.wasm, runtime: runtimeSrc, app: appSrc, css });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([html], { type: "text/html" }));
    a.download = "index.html";
    a.click();
    URL.revokeObjectURL(a.href);
    setOutput({ kind: "ok", text: `Saved index.html (${Math.round(html.length / 1024)} KB): one file with the program, the runtime and the app host. Put it on any static host, or open it from disk.` });
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
            const project = projects.find((x) => `project:${x.name}` === e.target.value);
            if (project) {
              setFiles(project.files);
              setActive(ENTRY);
            } else {
              const found = [...examples, ...rejects].find((x) => x.name === e.target.value);
              if (found) {
                setFiles({ [ENTRY]: found.source });
                setActive(ENTRY);
              }
            }
            e.target.value = "";
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
          <optgroup label="Multi-file projects">
            {projects.map((x) => (
              <option key={x.name} value={`project:${x.name}`}>
                {x.name} ({Object.keys(x.files).length} files)
              </option>
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
        <button onClick={() => runApp()} disabled={busy || diags.length > 0} title="Run init/update/view as an app">
          App
        </button>
        <button onClick={publish_} disabled={busy || diags.length > 0} title="Download the program as one self-contained web page">
          Publish
        </button>
        <button onClick={verify} disabled={busy || diags.length > 0} title="Prove contracts, indexing, division and termination with Z3 in the browser">
          Verify
        </button>
        <button onClick={runTests} disabled={busy || diags.length > 0}>
          Test
        </button>
        <span className="muted spacer">
          Types, patterns and names are checked as you type. <b>Verify</b> proves contracts with Z3 in your browser.
        </span>
      </header>
      <main className="panes">
        <section className="pane editor" aria-label="Source editor">
          <div className="tabs" role="tablist" aria-label="Project files">
            {Object.keys(files)
              .sort((a, b) => (a === ENTRY ? -1 : b === ENTRY ? 1 : a.localeCompare(b)))
              .map((name) => (
                <span key={name} className={`tab ${name === active ? "on" : ""}`}>
                  <button role="tab" aria-selected={name === active} onClick={() => setActive(name)}>
                    {name}
                    {diags.some((d) => (d.file ?? ENTRY) === name) ? <span className="bad"> ●</span> : null}
                  </button>
                  {name !== ENTRY && (
                    <button
                      className="x"
                      aria-label={`Delete ${name}`}
                      onClick={() => {
                        if (!confirm(`Delete ${name}?`)) return;
                        setFiles((f) => Object.fromEntries(Object.entries(f).filter(([k]) => k !== name)));
                        if (active === name) setActive(ENTRY);
                      }}
                    >
                      ×
                    </button>
                  )}
                </span>
              ))}
            <button
              className="add"
              title={'Add a file; use "name.tez" in another file pulls it in'}
              onClick={() => {
                const name = prompt("New file (relative path, e.g. util/money.tez)")?.trim();
                if (!name) return;
                const problem = validName(name, files);
                if (problem) return alert(problem);
                setFiles((f) => ({ ...f, [name]: "" }));
                setActive(name);
              }}
            >
              + file
            </button>
          </div>
          <Editor
            path={active}
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
                  {d.file ? `${d.file} · ` : ""}line {d.span.start.line}, column {d.span.start.col}
                  {d.expected ? ` · expected ${d.expected}` : ""}
                </div>
                {(d.file ?? ENTRY) !== active ? (
                  <button className="fix" onClick={() => setActive(d.file ?? ENTRY)}>
                    Open {d.file ?? ENTRY}
                  </button>
                ) : (
                  (d.fixes ?? []).map((f, j) => (
                    <button key={j} className="fix" onClick={() => applyFix(f)}>
                      Fix: {f.title}
                    </button>
                  ))
                )}
              </li>
            ))}
          </ul>
          <details className="ai">
            <summary>Ask a model</summary>
            <p className="muted">
              Any OpenAI-compatible server (Ollama, llama.cpp, vLLM, a hosted API). The language card is sent with the task,
              and compiler errors are fed back for up to 3 turns. Settings stay in this browser.
            </p>
            <label className="muted">
              <input type="checkbox" checked={aiLocal} disabled={!webGpuAvailable()} onChange={(e) => setAiLocal(e.target.checked)} /> Run the model in this browser (WebGPU
              {webGpuAvailable() ? "" : ": not available here"}; experimental, downloads about 4.9 GB the first time)
            </label>
            <input aria-label="Server URL" placeholder="http://localhost:11434/v1" value={ai.baseUrl ?? ""} onChange={(e) => setAi({ ...ai, baseUrl: e.target.value })} />
            <input aria-label="Model" placeholder="model name" value={ai.model ?? ""} onChange={(e) => setAi({ ...ai, model: e.target.value })} />
            <input aria-label="API key" type="password" placeholder="API key (if the server needs one)" value={ai.apiKey ?? ""} onChange={(e) => setAi({ ...ai, apiKey: e.target.value })} />
            <textarea aria-label="Task" rows={3} placeholder="What should the program do?" value={aiTask} onChange={(e) => setAiTask(e.target.value)} />
            <button onClick={askAi} disabled={busy}>
              Generate
            </button>
            <pre className="out">{aiLog}</pre>
          </details>
          <h2 hidden={!appOn}>App</h2>
          <div className="app" ref={appRoot} hidden={!appOn} />
          <h2>Output</h2>
          <pre className={`out ${output.kind}`}>{output.text}</pre>
        </section>
      </main>
    </div>
  );
}
