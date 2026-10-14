import "./style.css";
import counter from "../../spec/examples/15-counter-app.tez?raw";
import { appRunner, check, formatDiagnostics, highlight } from "./tez";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

// ---- live editor + app
const src = $<HTMLTextAreaElement>("src");
const hl = $("hl");
const status = $("status");
src.value = counter;

const paint = () => {
  hl.innerHTML = `${highlight(src.value)}\n`;
};
src.addEventListener("scroll", () => {
  hl.scrollTop = src.scrollTop;
  hl.scrollLeft = src.scrollLeft;
});

const start = appRunner($("app"), (text, ok) => {
  status.textContent = text.split("\n")[0] ?? "";
  status.className = `status ${ok ? "ok" : "bad"}`;
});

let timer: ReturnType<typeof setTimeout> | undefined;
src.addEventListener("input", () => {
  paint();
  clearTimeout(timer);
  timer = setTimeout(() => void start(src.value), 350);
});
paint();
void start(src.value);

// ---- highlight static snippets
for (const el of document.querySelectorAll<HTMLElement>("pre[data-tez]")) {
  el.innerHTML = highlight(el.textContent ?? "");
}

// ---- "forget a case" demo
const miss = $("miss");
$("miss-run").addEventListener("click", async () => {
  const ds = await check(miss.textContent ?? "");
  $("miss-out").textContent = ds.length ? formatDiagnostics(ds) : "no diagnostics";
});
