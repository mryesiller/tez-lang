// Contamination check: no training text may overlap a TezBench task.
//   node data/contamination.mjs FILE.jsonl [--field completion]   exit 1 when any row overlaps
//
// A row overlaps a task when its program is the task's reference solution after normalization, or shares a long
// run of tokens with it (default 24 consecutive tokens, i.e. most of a short function), or its text contains the
// task's prompt sentence.
import { readFileSync } from "node:fs";
import { tasks } from "../bench/tezbench/tasks.mjs";
import { tokens } from "./lib.mjs";

const NGRAM = 24;

function grams(toks, n) {
  const out = new Set();
  for (let i = 0; i + n <= toks.length; i++) out.add(toks.slice(i, i + n).join(" "));
  return out;
}

const benchGrams = tasks.map((t) => ({ id: t.id, prompt: t.prompt.toLowerCase(), toks: tokens(t.tez), grams: grams(tokens(t.tez), NGRAM) }));

/** @returns {string[]} ids of the TezBench tasks the program overlaps */
export function overlaps(program) {
  const toks = tokens(program);
  const mine = grams(toks, NGRAM);
  const hits = new Set();
  const lower = program.toLowerCase();
  for (const b of benchGrams) {
    const same = toks.length === b.toks.length && toks.every((x, i) => x === b.toks[i]);
    const shared = b.toks.length < NGRAM ? same : [...mine].some((g) => b.grams.has(g));
    // short references: a program that contains the whole reference as a sub-sequence
    const contains = b.toks.length < NGRAM && toks.join(" ").includes(b.toks.join(" "));
    if (same || shared || contains || lower.includes(b.prompt)) hits.add(b.id);
  }
  return [...hits];
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const file = process.argv[2];
  const field = process.argv.includes("--field") ? process.argv[process.argv.indexOf("--field") + 1] : "completion";
  if (!file) {
    console.error("usage: contamination.mjs FILE.jsonl [--field completion]");
    process.exit(2);
  }
  const rows = readFileSync(file, "utf8").split("\n").filter(Boolean).map((l) => JSON.parse(l));
  let bad = 0;
  rows.forEach((r, i) => {
    const hit = overlaps(String(r[field] ?? ""));
    if (hit.length) {
      bad++;
      console.log(`row ${i + 1}: overlaps ${hit.join(", ")}`);
    }
  });
  console.log(bad === 0 ? `clean: ${rows.length} rows, 0 overlaps with TezBench` : `${bad} of ${rows.length} rows overlap TezBench`);
  process.exit(bad === 0 ? 0 : 1);
}
