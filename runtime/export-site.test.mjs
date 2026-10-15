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
