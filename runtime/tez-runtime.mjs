// Runtime for Tez programs compiled to WasmGC (see crates/tez_wasm).
//
// Values are wasm `anyref`s: small Int = i31 (a JS Number), large Int = BigInt, Bool = 1/0,
// Unit = 0, Str = JS string, List = JS Array, Float = wasm struct (read via __f_get),
// records/ADTs/tuples = wasm structs (read via __tag/__len/__getN). Natives live here.

const FIT = 2n ** 30n;
const norm = (b) => (b >= -FIT && b < FIT ? Number(b) : b);
const big = (x) => (typeof x === "bigint" ? x : BigInt(x));
const cmpBig = (a, b) => {
  const x = big(a);
  const y = big(b);
  return x < y ? -1 : x > y ? 1 : 0;
};

/** A run-time failure with a stable Tez error code. */
export class TezError extends Error {
  constructor(code, message) {
    super(message);
    this.code = code;
  }
}

const MAX_LIST = 50_000_000;

function codePoints(s) {
  return Array.from(s);
}

function cmpStr(a, b) {
  // compare by code point, as Rust compares UTF-8 bytes
  const x = codePoints(a);
  const y = codePoints(b);
  const n = Math.min(x.length, y.length);
  for (let i = 0; i < n; i++) {
    const p = x[i].codePointAt(0);
    const q = y[i].codePointAt(0);
    if (p !== q) return p < q ? -1 : 1;
  }
  return x.length === y.length ? 0 : x.length < y.length ? -1 : 1;
}

function fmtFloat(f) {
  const s = String(f);
  return Number.isFinite(f) && !/[.eE]/.test(s) ? `${s}.0` : s;
}

function rustStr(s) {
  let out = '"';
  for (const c of s) {
    const p = c.codePointAt(0);
    if (c === '"') out += '\\"';
    else if (c === "\\") out += "\\\\";
    else if (c === "\n") out += "\\n";
    else if (c === "\r") out += "\\r";
    else if (c === "\t") out += "\\t";
    else if (c === "\0") out += "\\0";
    else if (p < 0x20 || p === 0x7f) out += `\\u{${p.toString(16)}}`;
    else out += c;
  }
  return `${out}"`;
}

export async function load(bytes) {
  const module = await WebAssembly.compile(bytes);
  const section = WebAssembly.Module.customSections(module, "tez-meta")[0];
  const meta = JSON.parse(new TextDecoder().decode(section));
  let ex = null; // the instance's exports, set after instantiation

  const some = (x) => ex.__some(x);
  const none = () => ex.__none();
  const num = (x) => Number(big(x));
  const newFloat = (f) => ex.__f_new(f);
  const getFloat = (x) => ex.__f_get(x);
  const idx = (i, len, what) => {
    const n = typeof i === "number" ? i : Number(i);
    if (!Number.isInteger(n) || n < 0 || n >= len) throw new TezError("E_INDEX_BOUNDS", `${what}: index out of range`);
    return n;
  };
  const nonNeg = (x, what) => {
    const b = big(x);
    if (b < 0n) throw new TezError("E_PRECONDITION", `${what} needs a non-negative count`);
    if (b > BigInt(MAX_LIST)) throw new TezError("E_RESOURCE", `${what}: too large`);
    return Number(b);
  };
  const truthy = (v) => v !== 0 && v !== false;

  function deepEq(a, b) {
    if (a === b) return true;
    if (typeof a === "number" || typeof a === "bigint" || typeof a === "string") return false;
    if (Array.isArray(a)) {
      return Array.isArray(b) && a.length === b.length && a.every((x, i) => deepEq(x, b[i]));
    }
    if (ex.__is_f(a)) return ex.__is_f(b) && getFloat(a) === getFloat(b);
    if (ex.__is_obj(a)) {
      if (!ex.__is_obj(b) || ex.__tag(a) !== ex.__tag(b) || ex.__len(a) !== ex.__len(b)) return false;
      for (let j = 0; j < ex.__len(a); j++) {
        if (!deepEq(ex[`__get${j}`](a), ex[`__get${j}`](b))) return false;
      }
      return true;
    }
    return false;
  }

  const natives = {
    "Int.pow": (a, e) => {
      const ex_ = big(e);
      if (ex_ < 0n) throw new TezError("E_PRECONDITION", "Int.pow needs a non-negative exponent");
      const base = big(a);
      if (ex_ > 200000n && (base > 1n || base < -1n)) throw new TezError("E_RESOURCE", "exponent too large");
      return norm(base ** ex_);
    },
    "Int.toStr": (a) => big(a).toString(),
    "Int.toFloat": (a) => {
      const f = Number(big(a));
      if (!Number.isFinite(f)) throw new TezError("E_RESOURCE", "integer too large for Float");
      return newFloat(f);
    },
    "Float.sqrt": (x) => {
      const v = getFloat(x);
      if (v < 0) throw new TezError("E_FLOAT_DOMAIN", "square root of a negative number");
      return newFloat(Math.sqrt(v));
    },
    "Float.floor": (x) => norm(BigInt(Math.floor(getFloat(x)))),
    "Float.ceil": (x) => norm(BigInt(Math.ceil(getFloat(x)))),
    "Float.round": (x) => {
      const v = getFloat(x);
      return norm(BigInt(Math.sign(v) * Math.floor(Math.abs(v) + 0.5)));
    },
    "Float.toStr": (x) => fmtFloat(getFloat(x)),

    "Str.len": (s) => codePoints(s).length,
    "Str.slice": (s, from, to) => {
      const cp = codePoints(s);
      const f = num(from);
      const t = num(to);
      if (!(f >= 0 && f <= t && t <= cp.length)) throw new TezError("E_PRECONDITION", "Str.slice range is out of bounds");
      return cp.slice(f, t).join("");
    },
    "Str.chars": (s) => codePoints(s),
    "Str.toInt": (s) => (/^-?[0-9]+$/.test(s) ? some(norm(BigInt(s))) : none()),
    "Str.contains": (s, p) => (s.includes(p) ? 1 : 0),
    "Str.startsWith": (s, p) => (s.startsWith(p) ? 1 : 0),
    "Str.endsWith": (s, p) => (s.endsWith(p) ? 1 : 0),
    "Str.trim": (s) => s.trim(),
    "Str.upper": (s) => s.toUpperCase(),
    "Str.lower": (s) => s.toLowerCase(),
    "Str.split": (s, sep) => {
      if (sep === "") throw new TezError("E_PRECONDITION", "Str.split needs a non-empty separator");
      return s.split(sep);
    },
    "Str.repeat": (s, n) => {
      const k = nonNeg(n, "Str.repeat");
      if (k * s.length > MAX_LIST) throw new TezError("E_RESOURCE", "string too large");
      return s.repeat(k);
    },

    "List.len": (l) => l.length,
    "List.get": (l, i) => l[idx(i, l.length, "List.get")],
    "List.range": (lo, hi) => {
      const a = big(lo);
      const b = big(hi);
      if (b <= a) return [];
      if (b - a > BigInt(MAX_LIST)) throw new TezError("E_RESOURCE", "range too large");
      const out = [];
      for (let i = a; i < b; i++) out.push(norm(i));
      return out;
    },
    "List.repeat": (x, n) => new Array(nonNeg(n, "List.repeat")).fill(x),
    "List.map": (l, f) => l.map((x) => ex.__apply1(f, x)),
    "List.filter": (l, f) => l.filter((x) => truthy(ex.__apply1(f, x))),
    "List.fold": (l, init, f) => l.reduce((acc, x) => ex.__apply2(f, acc, x), init),
    "List.flatMap": (l, f) => l.flatMap((x) => ex.__apply1(f, x)),
    "List.append": (l, x) => [...l, x],
    "List.concat": (a, b) => [...a, ...b],
    "List.reverse": (l) => [...l].reverse(),
    "List.take": (l, n) => {
      const k = big(n);
      return k <= 0n ? [] : l.slice(0, k > BigInt(l.length) ? l.length : Number(k));
    },
    "List.drop": (l, n) => {
      const k = big(n);
      return k <= 0n ? [...l] : k >= BigInt(l.length) ? [] : l.slice(Number(k));
    },
    "List.any": (l, f) => (l.some((x) => truthy(ex.__apply1(f, x))) ? 1 : 0),
    "List.all": (l, f) => (l.every((x) => truthy(ex.__apply1(f, x))) ? 1 : 0),
    "List.count": (l, f) => l.reduce((n, x) => n + (truthy(ex.__apply1(f, x)) ? 1 : 0), 0),
    "List.contains": (l, x) => (l.some((y) => deepEq(x, y)) ? 1 : 0),
    "List.indexOf": (l, x) => {
      const i = l.findIndex((y) => deepEq(x, y));
      return i < 0 ? none() : some(i);
    },
    "List.first": (l) => (l.length ? some(l[0]) : none()),
    "List.last": (l) => (l.length ? some(l[l.length - 1]) : none()),
    "List.zip": (a, b) => a.slice(0, Math.min(a.length, b.length)).map((x, i) => ex.__tup2(x, b[i])),
    "List.enumerate": (l) => l.map((x, i) => ex.__tup2(i, x)),
    "List.sortBy": (l, f) =>
      l
        .map((x) => [ex.__apply1(f, x), x])
        .sort((p, q) => cmpBig(p[0], q[0]))
        .map((p) => p[1]),
    "List.sort": (l) => [...l].sort(cmpBig),
    "List.sum": (l) => norm(l.reduce((s, x) => s + big(x), 0n)),
    "List.min": (l) => (l.length ? some(l.reduce((m, x) => (cmpBig(x, m) < 0 ? x : m))) : none()),
    "List.max": (l) => (l.length ? some(l.reduce((m, x) => (cmpBig(x, m) > 0 ? x : m))) : none()),
    "List.join": (l, sep) => l.join(sep),
  };

  const env = {
    big_from_i64: (x) => norm(x),
    big_add: (a, b) => norm(big(a) + big(b)),
    big_sub: (a, b) => norm(big(a) - big(b)),
    big_mul: (a, b) => norm(big(a) * big(b)),
    big_div: (a, b) => {
      const d = big(b);
      if (d === 0n) throw new TezError("E_DIV_ZERO", "division by zero");
      return norm(big(a) / d);
    },
    big_rem: (a, b) => {
      const d = big(b);
      if (d === 0n) throw new TezError("E_DIV_ZERO", "division by zero");
      return norm(big(a) % d);
    },
    big_neg: (a) => norm(-big(a)),
    big_lt: (a, b) => (big(a) < big(b) ? 1 : 0),
    big_le: (a, b) => (big(a) <= big(b) ? 1 : 0),
    big_eq: (a, b) => (big(a) === big(b) ? 1 : 0),
    big_lit: (i) => norm(BigInt(meta.bigs[i])),
    str_lit: (i) => meta.strs[i],
    str_eq: (a, b) => (a === b ? 1 : 0),
    str_lt: (a, b) => (cmpStr(a, b) < 0 ? 1 : 0),
    str_le: (a, b) => (cmpStr(a, b) <= 0 ? 1 : 0),
    str_concat: (a, b) => a + b,
    list_new: () => [],
    list_push: (l, x) => {
      l.push(x);
      return l;
    },
    list_len: (l) => l.length,
    list_get: (l, i) => {
      if (i < 0 || i >= l.length) throw new TezError("E_INDEX_BOUNDS", "index out of range");
      return l[i];
    },
    list_slice: (l, n) => l.slice(n),
    deep_eq: (a, b) => (deepEq(a, b) ? 1 : 0),
    fail: (code) => {
      throw new TezError(code === 0 ? "E_ASSERT" : "E_INTERNAL", code === 0 ? "assertion failed" : "internal failure");
    },
  };
  for (const [name, fn] of Object.entries(natives)) env[name] = fn;

  // the module imports exactly what it uses; refuse to start if the runtime lacks something
  const needed = WebAssembly.Module.imports(module).map((i) => i.name);
  const missing = needed.filter((n) => !(n in env));
  if (missing.length) throw new TezError("E_INTERNAL", `runtime lacks: ${missing.join(", ")}`);

  const instance = await WebAssembly.instantiate(module, { env });
  ex = instance.exports;

  // ---- reading values back

  const subst = (ty, params, args) => {
    if (ty.k === "param") {
      const i = params.indexOf(ty.n);
      return i >= 0 ? args[i] : ty;
    }
    if (ty.k === "list") return { k: "list", t: subst(ty.t, params, args) };
    if (ty.k === "tuple") return { k: "tuple", ts: ty.ts.map((t) => subst(t, params, args)) };
    if (ty.k === "adt") return { k: "adt", id: ty.id, args: ty.args.map((t) => subst(t, params, args)) };
    return ty;
  };

  function show(v, ty) {
    switch (ty.k) {
      case "int":
        return String(v);
      case "bool":
        return v === 1 ? "true" : "false";
      case "unit":
        return "()";
      case "float":
        return fmtFloat(getFloat(v));
      case "str":
        return rustStr(v);
      case "list":
        return `[${v.map((x) => show(x, ty.t)).join(", ")}]`;
      case "tuple":
        return `(${ty.ts.map((t, j) => show(ex[`__get${j}`](v), t)).join(", ")})`;
      case "adt": {
        const def = meta.types[ty.id];
        const variant = def.record ? def.variants[0] : def.variants[ex.__tag(v)];
        const fields = variant.tys.map((t, j) => show(ex[`__get${j}`](v), subst(t, def.params, ty.args)));
        if (def.record) {
          return `${def.name} { ${variant.fields.map((n, j) => `${n}: ${fields[j]}`).join(", ")} }`;
        }
        return fields.length ? `${variant.name}(${fields.join(", ")})` : variant.name;
      }
      default:
        return "<function>";
    }
  }

  const MAP = [
    [/divide by zero|remainder by zero/i, "E_DIV_ZERO"],
    [/call stack|stack overflow/i, "E_RESOURCE"],
  ];

  /** Calls the zero-argument Tez function `name` and renders its value like the interpreter. */
  function run(name) {
    const info = meta.fns[name];
    if (!info) throw new TezError("E_INTERNAL", `no function named ${name}`);
    try {
      const v = ex[`f_${name}`]();
      return show(v, info.ret);
    } catch (e) {
      if (e instanceof TezError) throw e;
      const msg = String(e && e.message ? e.message : e);
      const hit = MAP.find(([re]) => re.test(msg));
      throw new TezError(hit ? hit[1] : "E_INTERNAL", msg);
    }
  }

  return { exports: ex, meta, run, show };
}
