// A model that runs inside this browser tab (WebGPU, Transformers.js), as a provider for the repair loop.
// First use downloads about 4.9 GB (text-only q4f16 of Gemma 4 E4B); the browser caches it afterwards.
// Experimental: written from the model card's example and not exercised end to end here (that needs the download).

export const LOCAL_MODEL_ID = "onnx-community/gemma-4-E4B-it-ONNX";

type Loaded = {
  processor: { apply_chat_template(m: unknown, o: object): string; (p: string, i?: unknown, a?: unknown, o?: object): Promise<{ input_ids: { dims: number[] } }>; batch_decode(x: unknown, o: object): string[]; tokenizer: unknown };
  model: { generate(o: object): Promise<{ slice(a: unknown, b: unknown): unknown }> };
};

let loading: Promise<Loaded> | null = null;

export function webGpuAvailable(): boolean {
  return typeof navigator !== "undefined" && "gpu" in navigator;
}

async function load(onProgress: (text: string) => void): Promise<Loaded> {
  if (!webGpuAvailable()) throw new Error("this browser has no WebGPU");
  const t = await import("@huggingface/transformers");
  const processor = await t.AutoProcessor.from_pretrained(LOCAL_MODEL_ID);
  const model = await t.Gemma4ForConditionalGeneration.from_pretrained(LOCAL_MODEL_ID, {
    dtype: "q4f16",
    device: "webgpu",
    progress_callback: (info: { status?: string; progress?: number }) => {
      if (info.status === "progress_total") onProgress(`loading the model: ${Math.round(info.progress ?? 0)}%`);
    },
  });
  return { processor, model } as unknown as Loaded;
}

/** `async (prompt) => text`, like every other provider. Loads the model on first call. */
export function inBrowserModel(onProgress: (text: string) => void = () => {}, maxNewTokens = 700) {
  return async (prompt: string): Promise<string> => {
    loading ??= load(onProgress);
    const { processor, model } = await loading;
    const text = processor.apply_chat_template([{ role: "user", content: [{ type: "text", text: prompt }] }], {
      enable_thinking: false,
      add_generation_prompt: true,
    });
    const inputs = await processor(text, undefined, undefined, { add_special_tokens: false });
    const outputs = await model.generate({ ...inputs, max_new_tokens: maxNewTokens, do_sample: false });
    const promptLen = inputs.input_ids.dims.at(-1);
    return processor.batch_decode(outputs.slice(null, [promptLen, null]), { skip_special_tokens: true })[0] ?? "";
  };
}
