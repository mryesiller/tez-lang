// Download size of the browser (Transformers.js) build of the reference model, from Hugging Face metadata only
// (no weights are downloaded).   node bench/onnx-sizes.mjs [--json]
import { writeFileSync } from "node:fs";

const repo = "onnx-community/gemma-4-E4B-it-ONNX";
const res = await fetch(`https://huggingface.co/api/models/${repo}/tree/main/onnx`);
if (!res.ok) throw new Error(`HF API answered ${res.status}`);
const parts = {};
for (const f of await res.json()) {
  if (f.type !== "file") continue;
  const name = f.path.split("/").pop().split(".onnx")[0];
  parts[name] = (parts[name] ?? 0) + (f.lfs?.size ?? f.size ?? 0);
}
const mb = (b) => Math.round(b / 1e6);
const quants = ["q4f16", "q4", "quantized", "fp16"];
const rows = quants.map((q) => {
  const decoder = parts[`decoder_model_merged_${q}`] ?? 0;
  const embed = parts[`embed_tokens_${q}`] ?? 0;
  return { quant: q, decoderMB: mb(decoder), embedMB: mb(embed), textOnlyMB: mb(decoder + embed), visionMB: mb(parts[`vision_encoder_${q}`] ?? 0), audioMB: mb(parts[`audio_encoder_${q}`] ?? 0) };
});
console.log(`${repo}  (text-only = decoder + embed_tokens)`);
for (const r of rows) console.log(`${r.quant.padEnd(10)} decoder ${String(r.decoderMB).padStart(6)} MB  embed ${String(r.embedMB).padStart(6)} MB  text-only ${String(r.textOnlyMB).padStart(6)} MB`);
if (process.argv.includes("--json")) {
  const date = new Date().toISOString().slice(0, 10);
  const out = new URL(`./results/onnx-sizes-${date}.json`, import.meta.url).pathname;
  writeFileSync(out, JSON.stringify({ repo, date, rows }, null, 2));
  console.log(`wrote ${out}`);
}
