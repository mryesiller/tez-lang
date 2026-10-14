/** A run-time failure carrying a stable Tez error code (see spec/errors.md). */
export class TezError extends Error {
  code: string;
}

export interface TezRuntime {
  /** Calls the zero-argument Tez function and renders its value like the interpreter does. */
  run(name: string): string;
}

/** Instantiates a module produced by `tez build` / `tez_web.build`. */
export function load(bytes: Uint8Array): Promise<TezRuntime>;
