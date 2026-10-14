import { readFileSync } from "node:fs";
import { Tokenizer } from "@huggingface/tokenizers";
const json = JSON.parse(readFileSync("../cache/gemma4/tokenizer.json", "utf8"));
const cfgPath = "../cache/gemma4/tokenizer_config.json";
let cfg = {};
try { cfg = JSON.parse(readFileSync(cfgPath, "utf8")); } catch {}
const tok = new Tokenizer(json, cfg);
const e = tok.encode("fn add(a: Int, b: Int) -> Int = a + b");
console.log(Object.keys(e), e.tokens ?? e.ids);
