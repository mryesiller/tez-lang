#!/usr/bin/env node
// tez-mcp: a Model Context Protocol server (stdio, newline-delimited JSON-RPC 2.0) that exposes the Tez
// compiler to any MCP client: check, fmt, run, test. No dependencies. Set TEZ_BIN to the `tez` binary.
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInterface } from "node:readline";

const root = new URL("../../", import.meta.url).pathname;
const bin = process.env.TEZ_BIN ?? `${root}target/debug/tez`;
const dir = mkdtempSync(join(tmpdir(), "tez-mcp-"));
let counter = 0;

function tez(args, source) {
  const file = join(dir, `s${counter++}.tez`);
  writeFileSync(file, source);
  const r = spawnSync(bin, [args[0], ...args.slice(1), file], { encoding: "utf8", timeout: 60000 });
  return { code: r.status ?? 1, stdout: r.stdout ?? "", stderr: r.stderr ?? "", error: r.error };
}

const sourceProp = { type: "string", description: "Tez source code of one file" };
const tools = [
  {
    name: "tez_check",
    description:
      "Type-check a Tez program and prove its contracts. Returns JSON diagnostics: stable error code, position, expected form, counterexample and machine-applicable fixes. An empty list means the program is accepted and cannot fail in the proven classes.",
    inputSchema: { type: "object", properties: { source: sourceProp, verify: { type: "boolean", description: "Prove contracts with the SMT solver (default true)" } }, required: ["source"] },
    run: ({ source, verify = true }) => {
      const r = tez(["check", "--json", ...(verify ? [] : ["--no-verify"])], source);
      return { text: r.stdout || r.stderr, isError: r.code !== 0 && !r.stdout.includes('"diagnostics"') };
    },
  },
  {
    name: "tez_fmt",
    description: "Return the canonical formatting of a Tez program (the only accepted style).",
    inputSchema: { type: "object", properties: { source: sourceProp }, required: ["source"] },
    run: ({ source }) => {
      const r = tez(["fmt"], source);
      return { text: r.code === 0 ? r.stdout : r.stderr, isError: r.code !== 0 };
    },
  },
  {
    name: "tez_run",
    description: "Verify a Tez program, then evaluate a zero-argument function (default main) and return its value.",
    inputSchema: { type: "object", properties: { source: sourceProp, fn: { type: "string", description: "function to run (default main)" } }, required: ["source"] },
    run: ({ source, fn }) => {
      const file = join(dir, `s${counter++}.tez`);
      writeFileSync(file, source);
      const r = spawnSync(bin, ["run", file, ...(fn ? [fn] : [])], { encoding: "utf8", timeout: 60000 });
      return { text: r.status === 0 ? r.stdout : r.stderr, isError: r.status !== 0 };
    },
  },
  {
    name: "tez_test",
    description: "Run the `test` items of a Tez program and report pass/fail per test.",
    inputSchema: { type: "object", properties: { source: sourceProp }, required: ["source"] },
    run: ({ source }) => {
      const r = tez(["test"], source);
      return { text: r.stdout || r.stderr, isError: r.code !== 0 };
    },
  },
];

function reply(id, result) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`);
}
function fail(id, code, message) {
  process.stdout.write(`${JSON.stringify({ jsonrpc: "2.0", id, error: { code, message } })}\n`);
}

export function handle(msg) {
  const { id, method, params } = msg;
  if (id === undefined) return; // notifications need no answer
  switch (method) {
    case "initialize":
      return reply(id, {
        protocolVersion: params?.protocolVersion ?? "2025-06-18",
        capabilities: { tools: {} },
        serverInfo: { name: "tez-mcp", version: "0.0.1" },
        instructions: "Write Tez with tez_check until it returns no diagnostics; use tez_fmt for the canonical layout.",
      });
    case "ping":
      return reply(id, {});
    case "tools/list":
      return reply(id, { tools: tools.map(({ run, ...t }) => t) });
    case "tools/call": {
      const tool = tools.find((t) => t.name === params?.name);
      if (!tool) return fail(id, -32602, `unknown tool ${params?.name}`);
      const args = params.arguments ?? {};
      if (typeof args.source !== "string") return reply(id, { content: [{ type: "text", text: "`source` must be a string" }], isError: true });
      try {
        const { text, isError } = tool.run(args);
        return reply(id, { content: [{ type: "text", text }], isError });
      } catch (e) {
        return reply(id, { content: [{ type: "text", text: String(e) }], isError: true });
      }
    }
    default:
      return fail(id, -32601, `method not found: ${method}`);
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  createInterface({ input: process.stdin }).on("line", (line) => {
    if (!line.trim()) return;
    try {
      handle(JSON.parse(line));
    } catch {
      fail(null, -32700, "parse error");
    }
  });
}
