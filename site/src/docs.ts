import "./style.css";
import { marked } from "marked";
import language from "../../spec/language.md?raw";
import card from "../../spec/card.md?raw";
import errors from "../../spec/errors.md?raw";
import tour from "../../docs/TOUR.md?raw";
import architecture from "../../docs/ARCHITECTURE.md?raw";
import roadmap from "../../docs/ROADMAP.md?raw";
import { appRunner, check, formatDiagnostics, highlight, run } from "./tez";

const exampleFiles = import.meta.glob("../../spec/examples/*.tez", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
const examples = Object.entries(exampleFiles)
  .map(([path, source]) => ({ name: path.split("/").pop() ?? path, source }))
  .sort((a, b) => a.name.localeCompare(b.name));

type Page = { id: string; title: string; md?: string };

const PAGES: Page[] = [
  { id: "tour", title: "Tour", md: tour },
  { id: "language", title: "Language reference", md: language },
  { id: "card", title: "Language card (for models)", md: card },
  { id: "errors", title: "Error catalog", md: errors },
  { id: "examples", title: "Examples" },
  { id: "architecture", title: "Architecture", md: architecture },
  { id: "roadmap", title: "Roadmap", md: roadmap },
];

let currentId = "";
const side = document.getElementById("side") as HTMLElement;
const page = document.getElementById("page") as HTMLElement;

side.innerHTML = `<nav>${PAGES.map((p) => `<a href="#/${p.id}" data-id="${p.id}">${p.title}</a>`).join("")}</nav>`;

/** Wraps fenced code: Tez blocks get highlighting, and a Run button when they define `main`. */
function decorate(root: HTMLElement) {
  for (const code of root.querySelectorAll<HTMLElement>("pre > code")) {
    const text = code.textContent ?? "";
    const pre = code.parentElement as HTMLElement;
    const reject = code.className.includes("language-tez-reject");
    const isTez = reject || code.className.includes("language-tez") || (!code.className && /^\s*(fn|type|const|test)\s/m.test(text));
    if (!isTez) continue;
    code.innerHTML = highlight(text);
    pre.classList.add("code");
    if (reject) addChecker(pre, () => text.replace(/^\/\/ expect:.*\n/, ""));
    else if (/\bfn view\(/.test(text)) addApp(pre, () => text);
    else if (/\bfn main\(/.test(text)) addRunner(pre, () => text);
  }
}

function barWith(pre: HTMLElement, label: string, onClick: (btn: HTMLButtonElement, out: HTMLPreElement) => Promise<void>) {
  const bar = document.createElement("div");
  bar.className = "runbar";
  const btn = document.createElement("button");
  btn.className = "btn small";
  btn.textContent = label;
  const out = document.createElement("pre");
  out.className = "term out";
  out.hidden = true;
  btn.addEventListener("click", async () => {
    btn.disabled = true;
    await onClick(btn, out);
    btn.disabled = false;
  });
  bar.append(btn);
  pre.after(bar, out);
}

function addChecker(pre: HTMLElement, source: () => string) {
  barWith(pre, "Compile", async (_b, out) => {
    const ds = await check(source());
    out.hidden = false;
    out.className = `term out ${ds.length ? "bad" : "ok"}`;
    out.textContent = ds.length ? formatDiagnostics(ds) : "accepted";
  });
}

function addApp(pre: HTMLElement, source: () => string) {
  const host = document.createElement("div");
  host.className = "app-root live";
  host.hidden = true;
  pre.after(host);
  barWith(pre, "Run as app", async (_b, out) => {
    host.hidden = false;
    out.hidden = false;
    const go = appRunner(host, (text, ok) => {
      out.className = `term out ${ok ? "ok" : "bad"}`;
      out.textContent = text;
    });
    await go(source());
  });
}

function addRunner(pre: HTMLElement, source: () => string) {
  barWith(pre, "Run", async (_b, out) => {
    const r = await run(source());
    out.hidden = false;
    out.className = `term out ${r.ok ? "ok" : "bad"}`;
    out.textContent = r.ok ? `${r.value}\n(compiled to WebAssembly, ran in ${r.ms} ms)` : r.text;
  });
}

function renderExamples() {
  const ok = examples.filter((e) => !e.name.endsWith(".reject.tez"));
  const bad = examples.filter((e) => e.name.endsWith(".reject.tez"));
  page.innerHTML = `<h1>Examples</h1>
    <p>Every program below is part of the repository's test suite. Accepted programs are verified, run by the interpreter and by the compiled WebAssembly, and must produce the same value. Rejected programs must fail with exactly the code named in their first line.</p>
    <h2>Programs that are accepted</h2><div id="ok"></div>
    <h2>Programs that are rejected on purpose</h2><div id="bad"></div>`;
  const fill = (id: string, list: typeof examples) => {
    const host = page.querySelector(`#${id}`) as HTMLElement;
    for (const e of list) {
      const h = document.createElement("h3");
      h.textContent = e.name;
      const pre = document.createElement("pre");
      pre.className = "code";
      pre.innerHTML = highlight(e.source.trimEnd());
      host.append(h, pre);
      if (id === "ok" && /\bfn view\(/.test(e.source)) addApp(pre, () => e.source);
      else if (id === "ok" && /\bfn main\(/.test(e.source)) addRunner(pre, () => e.source);
      if (id === "bad") addChecker(pre, () => e.source.replace(/^\/\/ expect:.*\n/, ""));
    }
  };
  fill("ok", ok);
  fill("bad", bad);
}

function show() {
  const id = location.hash.replace(/^#\//, "") || "tour";
  const p = PAGES.find((x) => x.id === id) ?? PAGES[0];
  currentId = p.id;
  for (const a of side.querySelectorAll("a")) a.classList.toggle("on", a.dataset.id === p.id);
  document.title = `${p.title} · Tez documentation`;
  if (p.md === undefined) renderExamples();
  else {
    page.innerHTML = marked.parse(p.md, { async: false }) as string;
    decorate(page);
  }
  if (currentId) window.scrollTo(0, 0);
}

window.addEventListener("hashchange", show);
show();
