import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { siteHtml } from "./export-site.mjs";

const runtime = readFileSync(new URL("./tez-runtime.mjs", import.meta.url), "utf8");
const app = readFileSync(new URL("./tez-app.mjs", import.meta.url), "utf8");

test("the page is self-contained and its inlined script is valid JavaScript", async () => {
  const html = siteHtml({ title: "A <b> site", wasm: "AGFzbQ==", runtime, app });
  assert.match(html, /<title>A &lt;b&gt; site<\/title>/);
  assert.doesNotMatch(html, /src=|href=|import\s.*from|^export /m);
  assert.match(html, /Content-Security-Policy/);
  const script = html.split('<script type="module">')[1].split("</script>")[0];
  assert.doesNotThrow(() => new Function(`return (async () => {${script.replace(/\bawait load\b/, "load")}})`));
  assert.equal(script.includes("</script"), false);
});

test("titles and styles cannot close their elements, and every network read is limited to allowed origins", () => {
  const html = siteHtml({
    title: "x</script><script>alert(1)</script>",
    wasm: "AGFzbQ==",
    runtime,
    app,
    css: "p{}</style><img src=x onerror=alert(2)>",
    allow: ["https://api.example/v1"],
  });
  assert.equal(html.split("</script>").length, 2); // only the page's own script element closes
  assert.equal(html.split("</style>").length, 2);
  const csp = /Content-Security-Policy" content="([^"]*)"/.exec(html)[1];
  assert.match(csp, /img-src data: 'self' https:\/\/api\.example;/);
  assert.match(csp, /media-src data: 'self' https:\/\/api\.example;/);
  assert.match(csp, /connect-src 'self' https:\/\/api\.example$/);
  assert.doesNotMatch(csp, /https:(;| |$)/); // no blanket https: source
  assert.throws(() => siteHtml({ title: "t", wasm: "AGFzbQ==", runtime, app, allow: ["javascript:alert(1)"] }));
  assert.throws(() => siteHtml({ title: "t", wasm: '";alert(1)//', runtime, app }));
});
