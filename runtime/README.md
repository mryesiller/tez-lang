# runtime/

JavaScript that runs compiled Tez (WasmGC) in Node and in browsers.

- `tez-runtime.mjs` — the prelude's `extern` functions, big integers, structural equality, value printing, and the
  mapping of traps to stable error codes (`load(bytes)` → `{ run, show, exports, meta }`).
- `tez-app.mjs` — the Elm-architecture host for apps (`init`/`update`/`view`, optional `subs` for timers): vnode diff,
  click/input events, hot-swap by event replay, and an HTML sandbox (`safeTag`, `safeAttr`) so programs cannot run
  JavaScript in the page. Tests: `node --test runtime/tez-app.test.mjs`.
- `export-site.mjs` — one self-contained web page for a compiled program (Studio Publish, `tez site`).
- `run.mjs` — `node runtime/run.mjs out.wasm [fn]` runs a module built with `tez build`.
