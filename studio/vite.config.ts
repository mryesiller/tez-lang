import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Cross-origin isolation is required for SharedArrayBuffer (Z3 WASM, threaded tooling).
// Any production host must send the same two headers.
const isolation = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  plugins: [react()],
  server: { headers: isolation },
  preview: { headers: isolation },
  worker: { format: "es" },
});
