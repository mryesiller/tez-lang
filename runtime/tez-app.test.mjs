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
