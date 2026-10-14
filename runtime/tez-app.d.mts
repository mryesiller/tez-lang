import type { TezRuntime } from "./tez-runtime.mjs";

export interface AppHandle {
  readonly model: unknown;
  dispatch(msg: unknown): void;
  stop(): void;
}

/** True when the compiled program defines `init()`, `update(model, msg)` and `view(model)`. */
export function isApp(rt: TezRuntime): boolean;

/** Runs the program as an Elm-architecture app inside `root`. */
export function mountApp(rt: TezRuntime, root: HTMLElement, opts?: { onError?: (e: Error) => void }): AppHandle;
