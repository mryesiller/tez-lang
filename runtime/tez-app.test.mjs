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
