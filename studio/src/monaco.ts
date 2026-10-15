// Bundle Monaco locally: no CDN, so Studio works offline and under COEP.
import { loader } from "@monaco-editor/react";
import * as monaco from "./monaco-lite.js";
import editorWorker from "monaco-editor/editor/editor.worker?worker";

self.MonacoEnvironment = {
  getWorker: () => new editorWorker(),
};

loader.config({ monaco });

// Provisional highlighting from the draft syntax in docs/ARCHITECTURE.md §3.
// The real definition is generated from spec/grammar.ebnf in Phase 0.
monaco.languages.register({ id: "tez", extensions: [".tez"] });
monaco.languages.setMonarchTokensProvider("tez", {
  keywords: [
    "fn", "type", "use", "let", "match", "if", "else", "where", "with",
    "requires", "ensures", "decreases", "true", "false",
  ],
  tokenizer: {
    root: [
      [/\/\/.*$/, "comment"],
      [/"([^"\\]|\\.)*"/, "string"],
      [/\d+(\.\d+)?/, "number"],
      [/[A-Z][A-Za-z0-9_]*/, "type.identifier"],
      [/[a-z_][A-Za-z0-9_]*/, { cases: { "@keywords": "keyword", "@default": "identifier" } }],
    ],
  },
});

export { monaco };
