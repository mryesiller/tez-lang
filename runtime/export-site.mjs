// One self-contained web page for a compiled Tez program: the WasmGC module (base64), the runtime and the app host are
// inlined, so the file works from any static host, from `file://` and offline. Used by the Studio ("Publish") and
// `tez site`. The page runs under a strict Content-Security-Policy: no foreign scripts, and every network read (fetches,
// images, media, CSS images) goes only to the page's own origin and the origins its author allowed. Links the visitor
// clicks are ordinary navigation and are not limited.

/** JSON for a value placed inside a <script> element: `<` is escaped, so no text can close the element. */
const scriptJson = (v) => JSON.stringify(v).replace(/</g, "\\u003c");

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
 * @param {{title: string, wasm: string, runtime: string, app: string, css?: string, allow?: string[]}} p
 *   `wasm` is base64; `runtime`/`app` are the texts of tez-runtime.mjs / tez-app.mjs; `css` styles the page;
 *   `allow` lists the extra origins the program may fetch from (the page's own origin is always allowed).
 */
export function siteHtml({ title, wasm, runtime, app, css = "", allow = [] }) {
  const origins = allow.map((o) => {
    const u = new URL(o);
    if (u.protocol !== "https:" && u.protocol !== "http:") throw new Error(`not a web origin: ${o}`);
    return u.origin;
  });
  if (!/^[A-Za-z0-9+/=]*$/.test(wasm)) throw new Error("the module must be base64");
  const reach = `'self'${origins.map((o) => ` ${o}`).join("")}`;
  const csp = `default-src 'none'; script-src 'unsafe-inline' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; img-src data: ${reach}; media-src data: ${reach}; connect-src ${reach}`;
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
${css.replace(/<\/style/gi, "<\\/style")}
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
    let storage = null;
    try { storage = window.localStorage; } catch { /* storage is blocked: nothing is remembered */ }
    mountApp(rt, root, {
      onError: (e) => { root.textContent = "The program stopped: " + e.message; },
      storage,
      storageKey: ${scriptJson(`tez-site:${title}`)},
      fetch: window.fetch.bind(window),
      allow: [location.origin, ...${scriptJson(origins)}],
    });
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
