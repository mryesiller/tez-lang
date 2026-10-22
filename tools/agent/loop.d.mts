export interface Diag {
  code: string;
  message: string;
  span?: { start: { line: number; col: number } };
  expected?: string;
  counterexample?: Record<string, string>;
  fixes?: { title: string }[];
}
export interface Turn {
  prompt: string;
  code: string;
  diagnostics: Diag[];
}
export function extractCode(text: string): string;
export function renderDiagnostics(diags: Diag[], limit?: number): string;
/** Whether a request is for a web page rather than a function. */
export function wantsApp(task: string): boolean;
export function solve(o: {
  task: string;
  card?: string;
  /** a complete example program shown after the card (used for web apps) */
  example?: string;
  generate: (prompt: string) => Promise<string>;
  check: (code: string) => Promise<Diag[]>;
  maxTurns?: number;
  onTurn?: (turn: Turn, index: number) => void;
}): Promise<{ ok: boolean; code: string; turns: Turn[] }>;
