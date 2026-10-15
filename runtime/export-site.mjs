// One self-contained web page for a compiled Tez program: the WasmGC module (base64), the runtime and the app host are
// inlined, so the file works from any static host, from `file://` and offline. Used by the Studio ("Publish") and
// `tez site`. The page runs under a strict Content-Security-Policy: no network, no foreign scripts.

const escapeHtml = (s) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** Turns an ES module's text into plain top-level code: no `export`, no relative imports. */
function inline(src) {
  return src
    .replace(/^import .*$/gm, "")
    .replace(/^export default /gm, "")
    .replace(/^export (async function|function|class|const|let|var)/gm, "$1")
    .replace(/^export \{[^}]*\};?$/gm, "")
    .replace(/<\/script/gi, "<\\/script");
}

/**
 * @param {{title: string, wasm: string, runtime: string, app: string, css?: string}} p
 *   `wasm` is base64; `runtime`/`app` are the texts of tez-runtime.mjs / tez-app.mjs; `css` styles the page.
 */
export function siteHtml({ title, wasm, runtime, app, css = "" }) {
  const csp = "default-src 'none'; script-src 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; img-src data: https:; media-src data: https:";
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="${csp}">
<title>${escapeHtml(title)}</title>
<style>
body { margin: 0; padding: 24px; font: 16px/1.5 system-ui, sans-serif; }
#app { max-width: 720px; margin: 0 auto; }
pre#out { white-space: pre-wrap; }
${css}
</style>
</head>
<body>
<div id="app" class="app"></div>
<script type="module">
${inline(runtime)}
${inline(app)}
const b64 = "${wasm}";
const root = document.getElementById("app");
try {
  const rt = await load(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)));
  if (isApp(rt)) {
    mountApp(rt, root, { onError: (e) => { root.textContent = "The program stopped: " + e.message; } });
  } else {
    const pre = document.createElement("pre");
    pre.id = "out";
    pre.textContent = rt.run("main");
    root.append(pre);
  }
} catch (e) {
  root.textContent = "Cannot start: " + (e && e.message);
}
</script>
</body>
</html>
`;
}
