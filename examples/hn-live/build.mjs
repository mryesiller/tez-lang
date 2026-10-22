#!/usr/bin/env node
// The cron job's build step: fetch the current top stories, write them into stories.tez, let the Tez compiler prove and
// test the page, and write the finished site to dist/. Fails (exit 1) when anything is rejected, so a broken build is
// never deployed and the previous site stays up.
//   node build.mjs              uses the `tez` command (npm i -g tez-lang)
//   TEZ="node /path/to/bin/tez.mjs" node build.mjs
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";

const API = "https://hacker-news.firebaseio.com/v0";
const tez = (process.env.TEZ ?? "tez").split(" ");
const here = new URL(".", import.meta.url).pathname;

const get = async (path) => {
  const r = await fetch(`${API}/${path}`, { signal: AbortSignal.timeout(15000) });
  if (!r.ok) throw new Error(`${path}: HTTP ${r.status}`);
  return r.json();
};

const ids = (await get("topstories.json")).slice(0, 5);
const items = await Promise.all(ids.map((id) => get(`item/${id}.json`)));

// a Tez text literal: backslash and quote escaped, control characters dropped
const text = (s) => `"${String(s ?? "").replace(/[\u0000-\u001f]/g, " ").replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
const story = (it) => `Story { id: ${Number(it.id)}, title: ${text(it.title)}, url: ${text(it.url)}, score: ${Number(it.score ?? 0)}, by: ${text(it.by)} }`;

const builtAt = Math.floor(Date.now() / 1000);
writeFileSync(
  `${here}stories.tez`,
  `// Written by build.mjs at ${new Date(builtAt * 1000).toISOString()}. Do not edit.\nuse "story.tez"\n\nfn bakedAt() -> Int = ${builtAt}\n\nfn baked() -> List[Story] = [\n  ${items.map(story).join(",\n  ")}\n]\n`,
);

const run = (args) => {
  const r = spawnSync(tez[0], [...tez.slice(1), ...args], { cwd: here, stdio: "inherit" });
  if (r.status !== 0) {
    console.error(`build.mjs: \`tez ${args.join(" ")}\` failed`);
    process.exit(1);
  }
};
run(["check", "main.tez"]); // proves the program (contracts, indexing, division, termination)
run(["test", "main.tez"]);
mkdirSync(`${here}dist`, { recursive: true });
run(["site", "main.tez", "-o", "dist/index.html", "--allow=https://hacker-news.firebaseio.com", "--css=style.css", "--title=Hacker News, top 5"]);
console.log(`built dist/index.html with ${items.length} stories at ${new Date(builtAt * 1000).toISOString()}`);
