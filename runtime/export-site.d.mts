/** One self-contained web page for a compiled Tez program. `wasm` is base64; `runtime`/`app` are module texts. */
export function siteHtml(p: { title: string; wasm: string; runtime: string; app: string; css?: string }): string;
