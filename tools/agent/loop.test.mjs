import assert from "node:assert/strict";
import { test } from "node:test";
import { cliChecker } from "./checker.mjs";
import { extractCode, renderDiagnostics, solve } from "./loop.mjs";
import { openAICompatible, scripted } from "./providers.mjs";

test("extractCode takes the fenced block", () => {
  assert.equal(extractCode("Sure!\n```tez\nfn main() -> Int = 1\n```\nDone."), "fn main() -> Int = 1\n");
  assert.equal(extractCode("fn main() -> Int = 1"), "fn main() -> Int = 1\n");
});

test("the loop feeds real diagnostics back and converges", async () => {
  const seen = [];
  const result = await solve({
    task: "return the answer",
    generate: async (prompt) => {
      seen.push(prompt);
      return seen.length === 1 ? "```tez\nfn main() -> Int = \"42\"\n```" : "```tez\nfn main() -> Int = 42\n```";
    },
    check: cliChecker({ verify: false }),
  });
  assert.equal(result.ok, true);
  assert.equal(result.turns.length, 2);
  assert.match(seen[1], /E_TYPE_MISMATCH/, "the second prompt must contain the compiler's stable code");
  assert.match(seen[1], /fn main\(\) -> Int = "42"/, "and the rejected program");
});

test("the loop gives up after maxTurns", async () => {
  const result = await solve({ task: "x", generate: scripted(["fn main() -> Int = \"a\"", "fn main() -> Int = \"b\""]), check: cliChecker({ verify: false }), maxTurns: 2 });
  assert.equal(result.ok, false);
  assert.equal(result.turns.length, 2);
});

test("diagnostics render with code, position and expected form", () => {
  const text = renderDiagnostics([{ code: "E_X", message: "bad", span: { start: { line: 3, col: 4 } }, expected: "`y`" }]);
  assert.equal(text, "E_X at line 3, column 4: bad\n  expected: `y`");
});

test("the OpenAI-compatible provider speaks the chat protocol", async () => {
  let sent;
  const provider = openAICompatible({
    baseUrl: "http://x/v1/",
    model: "m",
    fetchImpl: async (url, init) => {
      sent = { url, body: JSON.parse(init.body) };
      return { ok: true, json: async () => ({ choices: [{ message: { content: "hi" } }] }) };
    },
  });
  assert.equal(await provider("p"), "hi");
  assert.equal(sent.url, "http://x/v1/chat/completions");
  assert.equal(sent.body.messages[0].content, "p");
});
