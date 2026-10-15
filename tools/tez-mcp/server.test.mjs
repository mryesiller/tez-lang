import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { test } from "node:test";
import { createInterface } from "node:readline";

function client() {
  const p = spawn("node", [new URL("./server.mjs", import.meta.url).pathname], { stdio: ["pipe", "pipe", "inherit"] });
  const waiting = new Map();
  createInterface({ input: p.stdout }).on("line", (l) => {
    const m = JSON.parse(l);
    waiting.get(m.id)?.(m);
  });
  let id = 0;
  return {
    call: (method, params) =>
      new Promise((resolve) => {
        const i = ++id;
        waiting.set(i, resolve);
        p.stdin.write(`${JSON.stringify({ jsonrpc: "2.0", id: i, method, params })}\n`);
      }),
    close: () => p.kill(),
  };
}

test("tez-mcp speaks MCP and wraps the compiler", async () => {
  const c = client();
  try {
    const init = await c.call("initialize", { protocolVersion: "2025-06-18", capabilities: {}, clientInfo: { name: "t", version: "0" } });
    assert.equal(init.result.serverInfo.name, "tez-mcp");
    const list = await c.call("tools/list");
    assert.deepEqual(list.result.tools.map((t) => t.name), ["tez_check", "tez_fmt", "tez_run", "tez_test"]);

    const bad = await c.call("tools/call", { name: "tez_check", arguments: { source: 'fn main() -> Int = "x"', verify: false } });
    assert.match(bad.result.content[0].text, /E_TYPE_MISMATCH/);

    const good = await c.call("tools/call", { name: "tez_check", arguments: { source: "fn main() -> Int = 1" } });
    assert.match(good.result.content[0].text, /"diagnostics": \[\]/);

    const run = await c.call("tools/call", { name: "tez_run", arguments: { source: "fn main() -> Int = 6 * 7" } });
    assert.equal(run.result.content[0].text.trim(), "42");

    const fmt = await c.call("tools/call", { name: "tez_fmt", arguments: { source: "fn   f( a:Int )->Int=a+1" } });
    assert.equal(fmt.result.content[0].text, "fn f(a: Int) -> Int = a + 1\n");

    const t = await c.call("tools/call", { name: "tez_test", arguments: { source: "fn f() -> Int = 1\ntest one = f() == 1" } });
    assert.match(t.result.content[0].text, /1 passed, 0 failed/);

    const unknown = await c.call("nope");
    assert.equal(unknown.error.code, -32601);
  } finally {
    c.close();
  }
});
