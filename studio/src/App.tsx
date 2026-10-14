import Editor from "@monaco-editor/react";
import { useState } from "react";

import "./monaco";

const SAMPLE = `// Draft syntax: see docs/ARCHITECTURE.md §3
fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  ensures lo <= result && result <= hi
  = if x < lo { lo } else if x > hi { hi } else { x }
`;

export default function App() {
  const [source, setSource] = useState(SAMPLE);

  return (
    <div className="studio">
      <header className="bar">
        <strong>Tez Studio</strong>
        <span className="muted">pre-alpha · compiler not connected (Phase 2)</span>
      </header>
      <main className="panes">
        <section className="pane" aria-label="Source editor">
          <Editor
            language="tez"
            theme="vs-dark"
            value={source}
            onChange={(value) => setSource(value ?? "")}
            options={{ minimap: { enabled: false }, fontSize: 14, tabSize: 2, automaticLayout: true }}
          />
        </section>
        <section className="pane side" aria-label="Diagnostics and preview">
          <h2>Diagnostics</h2>
          <p className="muted">
            Nothing is checked yet: this editor does not compile. Results will appear here once
            the compiler runs in a Worker.
          </p>
          <h2>Preview</h2>
          <p className="muted">The live view of the program appears here.</p>
        </section>
      </main>
    </div>
  );
}
