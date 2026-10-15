export function openAICompatible(o: {
  baseUrl: string;
  model: string;
  apiKey?: string;
  temperature?: number;
  maxTokens?: number;
  timeoutMs?: number;
  fetchImpl?: typeof fetch;
}): (prompt: string) => Promise<string>;
export function scripted(answers: string[]): () => Promise<string>;
