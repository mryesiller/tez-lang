import { defineConfig } from "vite";

// Relative base: the site works from a GitHub Pages sub-path or any static host.
export default defineConfig({
  base: "./",
  server: { fs: { allow: [".."] } },
  build: {
    target: "es2022",
    rollupOptions: {
      input: { main: new URL("./index.html", import.meta.url).pathname, docs: new URL("./docs/index.html", import.meta.url).pathname },
    },
  },
});
