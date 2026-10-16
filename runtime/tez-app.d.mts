import type { TezRuntime } from "./tez-runtime.mjs";

export interface AppHandle {
  readonly model: unknown;
  dispatch(msg: unknown): void;
  /**
   * Hot-swap: restart the new program from `init()` (or from the saved checkpoint the log starts at) and replay
   * the logged events. `restarted`: the checkpoint could not be restored, so the session started over.
   */
  swap(next: TezRuntime): { replayed: number; dropped: number; restarted: boolean };
  stop(): void;
}

/** True when the compiled program defines `init()`, `update(model, msg)` and `view(model)`. */
export function isApp(rt: TezRuntime): boolean;

/** Reads a response body as text, giving up (null) once it passes `limit` bytes. */
export function readLimited(response: Response, limit?: number): Promise<string | null>;

/** Runs the program as an Elm-architecture app inside `root`. */
export function mountApp(
  rt: TezRuntime,
  root: HTMLElement,
  opts?: {
    onError?: (e: Error) => void;
    /** enables the program's `save`/`restore` (e.g. localStorage) */
    storage?: { getItem(k: string): string | null; setItem(k: string, v: string): void } | null;
    storageKey?: string;
    /** enables the program's `fetches`, for the origins in `allow` only */
    fetch?: typeof fetch | null;
    allow?: string[];
  },
): AppHandle;
