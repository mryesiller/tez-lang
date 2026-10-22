import assert from "node:assert/strict";
import { test } from "node:test";
import { safeAttr, safeTag } from "./tez-app.mjs";

test("script-capable tags are not created", () => {
  for (const t of ["script", "SCRIPT", "iframe", "object", "embed", "style", "link", "meta", "base", "form", "svg", "math", "template"]) {
    assert.equal(safeTag(t), "div", t);
  }
  for (const t of ["div", "button", "li", "a", "img", "input"]) assert.equal(safeTag(t), t);
});

test("event handlers and script URLs are dropped", () => {
  assert.equal(safeAttr("onclick", "x()"), null);
  assert.equal(safeAttr("OnError", "x()"), null);
  assert.equal(safeAttr("srcdoc", "<script>"), null);
  assert.equal(safeAttr("href", "javascript:alert(1)"), null);
  assert.equal(safeAttr("href", " JaVa\tScRiPt:alert(1)"), null);
  assert.equal(safeAttr("src", "data:text/html,<script>"), null);
  assert.equal(safeAttr("xlink:href", "#x"), null);
  assert.equal(safeAttr("style", "background:url(https://evil/x)"), null);
});

test("ordinary attributes pass", () => {
  assert.equal(safeAttr("class", "row"), "row");
  assert.equal(safeAttr("href", "https://example.com"), "https://example.com");
  assert.equal(safeAttr("href", "#top"), "#top");
  assert.equal(safeAttr("href", "/docs/"), "/docs/");
  assert.equal(safeAttr("style", "color: red; width: 50%"), "color: red; width: 50%");
  assert.equal(safeAttr("data-id", "3"), "3");
});

import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { load } from "./tez-runtime.mjs";
import { readSubs, subMessage } from "./tez-app.mjs";

test("subscriptions: fixed messages, the clock and browser state", async () => {
  const root = new URL("../", import.meta.url).pathname;
  const bin = process.env.TEZ_BIN ?? `${root}target/debug/tez`;
  const src = `type Msg = Tick | At(Int) | Net(Bool) | Seen(Bool)
fn init() -> Int = 0
fn update(m: Int, msg: Msg) -> Int = match msg {
  Tick => m + 1
  At(t) => t
  Net(b) => if b { 1 } else { 0 }
  Seen(b) => if b { 10 } else { 20 }
}
fn view(m: Int) -> Html[Msg] = Html.text(m.toStr())
fn subs(m: Int) -> List[Sub[Msg]] =
  [Sub.every(100, Tick), Sub.clock(1000, t => At(t)), Sub.online(b => Net(b)), Sub.visible(b => Seen(b))]
`;
  const dir = mkdtempSync(join(tmpdir(), "tez-subs-"));
  writeFileSync(join(dir, "a.tez"), src);
  const r = spawnSync(bin, ["build", "--no-verify", join(dir, "a.tez"), "-o", join(dir, "a.wasm")], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const rt = await load(readFileSync(join(dir, "a.wasm")));
  const model = rt.exports.f_init();
  const subs = readSubs(rt, model);
  assert.deepEqual(subs.map((s) => s.kind), ["every", "clock", "online", "visible"]);
  const upd = (sub, v) => rt.exports.f_update(model, subMessage(rt, sub, v));
  assert.equal(upd(subs[0]), 1);
  assert.equal(upd(subs[1], 1_700_000_000), 1_700_000_000n); // bigger than 31 bits: a BigInt in WebAssembly
  assert.equal(upd(subs[1], 42), 42);
  assert.equal(upd(subs[2], 1), 1);
  assert.equal(upd(subs[3], 0), 20);
});

import { fetchAllowed, readFetches } from "./tez-app.mjs";

test("a program can fetch only from origins its page allows", () => {
  const base = "https://site.example/app/";
  const allow = ["https://site.example", "https://api.example"];
  assert.equal(fetchAllowed("data/x.txt", allow, base), "https://site.example/app/data/x.txt");
  assert.equal(fetchAllowed("https://api.example/v1?q=1", allow, base), "https://api.example/v1?q=1");
  assert.equal(fetchAllowed("https://evil.example/", allow, base), null);
  assert.equal(fetchAllowed("http://site.example/", allow, base), null); // another origin: scheme differs
  assert.equal(fetchAllowed("javascript:alert(1)", allow, base), null);
  assert.equal(fetchAllowed("data:text/plain,hi", allow, base), null);
  assert.equal(fetchAllowed("file:///etc/passwd", allow, base), null);
  assert.equal(fetchAllowed("https://site.example/", [], base), null);
});

test("fetches are read from the program", async () => {
  const root = new URL("../", import.meta.url).pathname;
  const bin = process.env.TEZ_BIN ?? `${root}target/debug/tez`;
  const dir = mkdtempSync(join(tmpdir(), "tez-fetch-"));
  writeFileSync(
    join(dir, "a.tez"),
    `type Msg = Good(Str) | Bad(Str)
fn init() -> Int = 0
fn update(m: Int, msg: Msg) -> Int = m
fn view(m: Int) -> Html[Msg] = Html.text("x")
fn fetches(m: Int) -> List[Fetch[Msg]] = [Fetch.get("a.txt", s => Good(s), e => Bad(e))]
`,
  );
  const r = spawnSync(bin, ["build", "--no-verify", join(dir, "a.tez"), "-o", join(dir, "a.wasm")], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const rt = await load(readFileSync(join(dir, "a.wasm")));
  const [f] = readFetches(rt, rt.exports.f_init());
  assert.equal(f.url, "a.txt");
  const msg = rt.exports.__apply1(f.ok, "body");
  assert.equal(rt.exports.__get0(msg), "body");
  assert.equal(rt.exports.__tag(rt.exports.__apply1(f.err, "x")), 1);
});

import { mountApp, readLimited } from "./tez-app.mjs";

test("a response body is read only up to the limit", async () => {
  assert.equal(await readLimited(new Response("héllo"), 10), "héllo");
  assert.equal(await readLimited(new Response("x".repeat(11)), 10), null);
  assert.equal(await readLimited(new Response("é".repeat(6)), 10), null); // 12 bytes, 6 characters
  const declared = new Response("small", { headers: { "content-length": "999999999" } });
  assert.equal(await readLimited(declared, 10), null);
  let pulled = 0;
  const endless = new ReadableStream({
    pull(c) {
      pulled++;
      c.enqueue(new Uint8Array(1024));
    },
  });
  assert.equal(await readLimited(new Response(endless), 4096), null);
  assert.ok(pulled < 10, `stopped reading after ${pulled} chunks`);
});

/** Just enough of a DOM for mountApp: elements, text nodes, attributes and click listeners. */
function fakeDocument() {
  class Node {
    constructor(doc) {
      this.ownerDocument = doc;
      this.childNodes = [];
      this.parentNode = null;
      this.listeners = {};
    }
    get lastChild() {
      return this.childNodes[this.childNodes.length - 1];
    }
    appendChild(n) {
      n.parentNode = this;
      this.childNodes.push(n);
      return n;
    }
    removeChild(n) {
      this.childNodes.splice(this.childNodes.indexOf(n), 1);
      n.parentNode = null;
      return n;
    }
    replaceChild(n, old) {
      this.childNodes[this.childNodes.indexOf(old)] = n;
      n.parentNode = this;
      old.parentNode = null;
      return old;
    }
    setAttribute() {}
    removeAttribute() {}
    addEventListener(type, f) {
      this.listeners[type] = f;
    }
    set textContent(t) {
      this.childNodes = [];
    }
    get text() {
      return this.nodeValue ?? this.childNodes.map((c) => c.text).join("");
    }
  }
  const doc = { defaultView: null, baseURI: "https://site.example/" };
  doc.createElement = (tag) => Object.assign(new Node(doc), { tag });
  doc.createTextNode = (t) => Object.assign(new Node(doc), { nodeValue: t });
  return { doc, root: new Node(doc) };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

async function counter(saveLine) {
  const root = new URL("../", import.meta.url).pathname;
  const bin = process.env.TEZ_BIN ?? `${root}target/debug/tez`;
  const dir = mkdtempSync(join(tmpdir(), "tez-log-"));
  writeFileSync(
    join(dir, "a.tez"),
    `type Msg = Inc | Reset
fn init() -> Int = 0
fn update(m: Int, msg: Msg) -> Int = match msg {
  Inc => m + 1
  Reset => 0
}
fn view(m: Int) -> Html[Msg] = Html.button(m.toStr(), Inc)
${saveLine}
`,
  );
  const r = spawnSync(bin, ["build", "--no-verify", join(dir, "a.tez"), "-o", join(dir, "a.wasm")], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  return () => load(readFileSync(join(dir, "a.wasm")));
}

test("a full replay log becomes a checkpoint, so a hot-swap still rebuilds the exact session", async () => {
  const build = await counter(`fn save(m: Int) -> Str = m.toStr()
fn restore(s: Str) -> Option[Int] = s.toInt()`);
  const { root } = fakeDocument();
  const app = mountApp(await build(), root, { logLimit: 3 });
  for (let i = 0; i < 10; i++) {
    root.childNodes[0].listeners.click();
    await flush();
  }
  assert.equal(root.text, "10");
  const r = app.swap(await build());
  assert.equal(r.restarted, false);
  assert.equal(r.dropped, 0);
  assert.equal(root.text, "10");
});

test("without save/restore, events past a full log are reported as dropped, never silently lost", async () => {
  const build = await counter("");
  const { root } = fakeDocument();
  const app = mountApp(await build(), root, { logLimit: 3 });
  for (let i = 0; i < 10; i++) {
    root.childNodes[0].listeners.click();
    await flush();
  }
  const r = app.swap(await build());
  assert.deepEqual([r.replayed, r.dropped], [3, 7]);
  assert.equal(root.text, "3"); // the state the kept log leads to, and the swap says so
});

import { requestAllowed } from "./tez-app.mjs";

test("requests: methods, headers and bodies are limited; requests are told apart by method, url and body", async () => {
  assert.deepEqual(requestAllowed("GET", [], ""), { headers: {} });
  assert.deepEqual(requestAllowed("POST", [["Content-Type", "application/json"]], "{}"), { headers: { "content-type": "application/json" } });
  for (const bad of [
    requestAllowed("TRACE", [], ""),
    requestAllowed("CONNECT", [], ""),
    requestAllowed("GET", [], "body"),
    requestAllowed("POST", [["Cookie", "a=b"]], "x"),
    requestAllowed("POST", [["Host", "evil"]], "x"),
    requestAllowed("POST", [["Sec-Fetch-Mode", "x"]], "x"),
    requestAllowed("POST", [["x", "a\r\nInjected: 1"]], "x"),
    requestAllowed("POST", [["bad name", "x"]], "x"),
    requestAllowed("POST", [], "x".repeat(70000)),
    requestAllowed("POST", Array.from({ length: 11 }, (_, i) => [`h${i}`, "v"]), "x"),
  ]) assert.ok(bad.error, JSON.stringify(bad));

  const root = new URL("../", import.meta.url).pathname;
  const bin = process.env.TEZ_BIN ?? `${root}target/debug/tez`;
  const dir = mkdtempSync(join(tmpdir(), "tez-req-"));
  writeFileSync(
    join(dir, "a.tez"),
    `type Msg = Good(Str) | Bad(Str)
fn init() -> Int = 0
fn update(m: Int, msg: Msg) -> Int = m
fn view(m: Int) -> Html[Msg] = Html.text("x")
fn fetches(m: Int) -> List[Fetch[Msg]] = [
  Fetch.get("a.txt", s => Good(s), e => Bad(e)),
  Fetch.post("save", "hello", s => Good(s), e => Bad(e)),
  Fetch.postJson("save", JObj([("n", JNum("1"))]), s => Good(s), e => Bad(e)),
  Fetch.send("PUT", "item/7", [("x-key", "k")], "v", s => Good(s), e => Bad(e))
]
`,
  );
  const r = spawnSync(bin, ["build", "--no-verify", join(dir, "a.tez"), "-o", join(dir, "a.wasm")], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const rt = await load(readFileSync(join(dir, "a.wasm")));
  const fs = readFetches(rt, rt.exports.f_init());
  assert.deepEqual(fs.map((f) => f.method), ["GET", "POST", "POST", "PUT"]);
  assert.equal(new Set(fs.map((f) => f.key)).size, 4); // the same url with another body is another request
  assert.deepEqual(fs[1].headers, [["content-type", "text/plain;charset=UTF-8"]]);
  assert.equal(fs[1].body, "hello");
  assert.equal(fs[2].body, '{"n":1}');
  assert.deepEqual(fs[3].headers, [["x-key", "k"]]);
});

test("a POST goes out once with its headers and body; the answer is delivered; blocked requests answer with a reason", async () => {
  const root = new URL("../", import.meta.url).pathname;
  const bin = process.env.TEZ_BIN ?? `${root}target/debug/tez`;
  const dir = mkdtempSync(join(tmpdir(), "tez-post-"));
  writeFileSync(
    join(dir, "a.tez"),
    `type Msg = Good(Str) | Bad(Str)
fn init() -> Str = ""
fn update(m: Str, msg: Msg) -> Str = match msg {
  Good(s) => m ++ "+" ++ s
  Bad(e) => m ++ "!" ++ e
}
fn view(m: Str) -> Html[Msg] = Html.text(m)
fn fetches(m: Str) -> List[Fetch[Msg]] = [
  Fetch.post("https://api.example/save", "payload", s => Good(s), e => Bad(e)),
  Fetch.send("TRACE", "https://api.example/t", [], "", s => Good(s), e => Bad(e)),
  Fetch.post("https://evil.example/x", "p", s => Good(s), e => Bad(e))
]
`,
  );
  const r = spawnSync(bin, ["build", "--no-verify", join(dir, "a.tez"), "-o", join(dir, "a.wasm")], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const calls = [];
  const fetchFn = async (url, init) => {
    calls.push({ url, method: init.method, body: init.body, headers: init.headers, credentials: init.credentials });
    return new Response("saved");
  };
  const { root: rootEl } = fakeDocument();
  mountApp(await load(readFileSync(join(dir, "a.wasm"))), rootEl, { fetch: fetchFn, allow: ["https://api.example"] });
  await new Promise((res) => setTimeout(res, 30));
  assert.equal(calls.length, 1); // TRACE and the foreign origin never reach the network
  assert.deepEqual(calls[0], { url: "https://api.example/save", method: "POST", body: "payload", headers: { "content-type": "text/plain;charset=UTF-8" }, credentials: "omit" });
  const shown = rootEl.text;
  assert.ok(shown.includes("+saved"), shown);
  assert.ok(shown.includes("!blocked: the method TRACE is not allowed"), shown);
  assert.ok(shown.includes("!blocked: this origin is not allowed"), shown);
});
