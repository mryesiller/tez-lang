// The repair loop: task -> model -> `check` -> diagnostics back to the model, until the program is accepted.
// Pure and provider-agnostic: the model and the checker are functions passed in, so the same loop runs in
// Node (tez CLI + any HTTP provider) and in the browser (Studio: in-page compiler + any HTTP provider).

/** Strips markdown fences and chatter around code. */
export function extractCode(text) {
  const fenced = [...text.matchAll(/```[a-zA-Z-]*\n([\s\S]*?)```/g)].map((m) => m[1]);
  if (fenced.length) return fenced.join("\n").trim() + "\n";
  return text.trim() + "\n";
}

/** The diagnostics a model sees: stable code, message, position, expected form, and offered fixes. */
export function renderDiagnostics(diags, limit = 8) {
  return diags
    .slice(0, limit)
    .map((d) => {
      const fix = (d.fixes ?? []).map((f) => f.title).join("; ");
      return [
        `${d.code} at line ${d.span?.start?.line ?? "?"}, column ${d.span?.start?.col ?? "?"}: ${d.message}`,
        d.expected ? `  expected: ${d.expected}` : "",
        d.counterexample ? `  counterexample: ${JSON.stringify(d.counterexample)}` : "",
        fix ? `  fix: ${fix}` : "",
      ]
        .filter(Boolean)
        .join("\n");
    })
    .join("\n");
}

export function firstPrompt({ card, task }) {
  return [
    "Write a Tez program. Reply with Tez code only, in one code block.",
    card ? `\n${card.trim()}\n` : "",
    `Task: ${task.trim()}`,
  ].join("\n");
}

export function repairPrompt({ card, task, code, diagnostics }) {
  return [
    "Your Tez program was rejected by the compiler. Fix every problem and reply with the complete corrected program in one code block.",
    card ? `\n${card.trim()}\n` : "",
    `Task: ${task.trim()}`,
    `\nYour program:\n\`\`\`\n${code.trim()}\n\`\`\``,
    `\nCompiler diagnostics:\n${renderDiagnostics(diagnostics)}`,
  ].join("\n");
}

/**
 * Runs the loop.
 * @param {object} o
 * @param {string} o.task        what to build, in words
 * @param {string} [o.card]      the language card (omit for the no-card baseline)
 * @param {(prompt: string) => Promise<string>} o.generate   the model
 * @param {(code: string) => Promise<Array<object>>} o.check  diagnostics for a program ([] = accepted)
 * @param {number} [o.maxTurns]
 * @param {(turn: object) => void} [o.onTurn]
 * @returns {Promise<{ok: boolean, code: string, turns: Array<{prompt: string, code: string, diagnostics: object[]}>}>}
 */
export async function solve({ task, card = "", generate, check, maxTurns = 3, onTurn = () => {} }) {
  const turns = [];
  let prompt = firstPrompt({ card, task });
  let code = "";
  for (let i = 0; i < maxTurns; i++) {
    code = extractCode(await generate(prompt));
    const diagnostics = await check(code);
    const turn = { prompt, code, diagnostics };
    turns.push(turn);
    onTurn(turn, i);
    if (diagnostics.length === 0) return { ok: true, code, turns };
    prompt = repairPrompt({ card, task, code, diagnostics });
  }
  return { ok: false, code, turns };
}
