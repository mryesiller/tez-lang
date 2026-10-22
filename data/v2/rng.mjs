// A small seeded random generator and the helpers the task families share.

export function rng(seed) {
  let s = seed >>> 0;
  const next = () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const r = {
    next,
    int: (lo, hi) => lo + Math.floor(next() * (hi - lo + 1)),
    pick: (xs) => xs[Math.floor(next() * xs.length)],
    chance: (p) => next() < p,
    /** a list of `n` integers in [lo, hi] */
    list: (n, lo, hi) => Array.from({ length: n }, () => lo + Math.floor(next() * (hi - lo + 1))),
    shuffle: (xs) => {
      const a = [...xs];
      for (let i = a.length - 1; i > 0; i--) {
        const j = Math.floor(next() * (i + 1));
        [a[i], a[j]] = [a[j], a[i]];
      }
      return a;
    },
  };
  return r;
}

/** A tuple value for `lit`. */
export const tup = (...items) => ({ tup: items });

/** A value as a Tez literal. */
export function lit(v) {
  if (typeof v === "string") return JSON.stringify(v);
  if (typeof v === "boolean") return String(v);
  if (typeof v === "number") return v < 0 ? `-${-v}` : String(v);
  if (Array.isArray(v)) return `[${v.map(lit).join(", ")}]`;
  if (v && v.tup) return `(${v.tup.map(lit).join(", ")})`;
  if (v && v.raw !== undefined) return v.raw;
  throw new Error(`cannot write ${JSON.stringify(v)}`);
}

/** Raw Tez text as a value (a constructor call, say). */
export const raw = (text) => ({ raw: text });

/** Integer division and remainder that truncate toward zero, as Tez does. */
export const idiv = (a, b) => Math.trunc(a / b);
export const imod = (a, b) => a - b * Math.trunc(a / b);
