// Model providers. A provider is `async (prompt: string) => string`. Nothing here is specific to one vendor.

/**
 * Any server that speaks the OpenAI chat-completions protocol (OpenAI, vLLM, llama.cpp server, Ollama, LM Studio,
 * OpenRouter, ...). Works in Node and in the browser.
 */
export function openAICompatible({ baseUrl, model, apiKey = "", temperature = 0.2, maxTokens = 1024, timeoutMs = 300000, fetchImpl = fetch }) {
  const url = `${baseUrl.replace(/\/$/, "")}/chat/completions`;
  return async (prompt) => {
    let res;
    try {
      res = await fetchImpl(url, {
      method: "POST",
      signal: AbortSignal.timeout(timeoutMs),
      headers: { "content-type": "application/json", ...(apiKey ? { authorization: `Bearer ${apiKey}` } : {}) },
        body: JSON.stringify({ model, temperature, max_tokens: maxTokens, messages: [{ role: "user", content: prompt }] }),
      });
    } catch (e) {
      // in a browser a refused cross-origin request only says "Failed to fetch"
      const inBrowser = typeof window !== "undefined";
      const hint = inBrowser
        ? " — the server is not running, or it does not allow requests from this page (CORS). In LM Studio: Developer → Server Settings → Enable CORS; in Ollama: set OLLAMA_ORIGINS"
        : " — is the server running?";
      throw new Error(`could not reach ${url}: ${e instanceof Error ? e.message : e}${hint}`);
    }
    if (!res.ok) throw new Error(`model server answered ${res.status}: ${(await res.text()).slice(0, 200)}`);
    const body = await res.json();
    const text = body?.choices?.[0]?.message?.content;
    if (typeof text !== "string") throw new Error("model server returned no message content");
    return text;
  };
}

/** A canned provider for tests and offline demos: answers are consumed in order. */
export function scripted(answers) {
  let i = 0;
  return async () => {
    if (i >= answers.length) throw new Error("scripted provider ran out of answers");
    return answers[i++];
  };
}
