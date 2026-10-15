// Fix traces without a model: take accepted programs, break them in one targeted way, let the real compiler
// say what is wrong, and keep (broken program, diagnostics, original program) when the compiler rejects it.
// The model later learns to read the stable diagnostic and produce the repair.
import { diagnose } from "./lib.mjs";

const lines = (s) => s.split("\n");
const join = (ls) => ls.join("\n");

/** Each operator returns a list of mutated sources (possibly empty) for one program. */
export const operators = {
  "drop-match-arm": (src) => {
    const ls = lines(src);
    const out = [];
    ls.forEach((l, i) => {
      // an arm line (`Pattern => value`) inside a match, whose next line is also an arm or the closing brace
      if (/^\s+[A-Z_\[(][^=]*=>/.test(l) && i + 1 < ls.length && /^\s*(\}|[A-Z_\[(])/.test(ls[i + 1])) out.push(join(ls.filter((_, j) => j !== i)));
    });
    return out;
  },
  "drop-decreases": (src) => {
    const ls = lines(src);
    return ls.flatMap((l, i) => (/^\s+decreases\b/.test(l) ? [join(ls.filter((_, j) => j !== i))] : []));
  },
  "drop-requires": (src) => {
    const ls = lines(src);
    return ls.flatMap((l, i) => (/^\s+requires\b/.test(l) ? [join(ls.filter((_, j) => j !== i))] : []));
  },
  "drop-ensures-weaken": (src) => {
    // make a postcondition false: `>= 0` -> `< 0`
    return src.includes("ensures") ? [src.replace(/(ensures [^\n]*?)>= (\d+)/, "$1< $2")].filter((s) => s !== src) : [];
  },
  "drop-else": (src) => {
    const m = src.match(/ else \{[^{}]*\}/);
    return m ? [src.replace(m[0], "")] : [];
  },
  "wrong-literal-type": (src) => {
    const m = src.match(/\b(\d+)\b(?!\s*\))/);
    return m && /= [^\n]*\b\d+\b/.test(src) ? [src.replace(/(= [^\n"]*?)\b(\d+)\b/, '$1"$2"')].filter((s) => s !== src) : [];
  },
  "unknown-name": (src) => {
    const call = src.match(/\b([a-z][A-Za-z0-9]*)\(/g)?.map((x) => x.slice(0, -1)).find((n) => !["fn", "if", "match"].includes(n) && new RegExp(`fn ${n}\\b`).test(src));
    return call ? [src.replace(new RegExp(`\\b${call}\\(`, "g"), `${call}X(`)] : [];
  },
  "snake-case": (src) => {
    const m = src.match(/\bfn ([a-z]+)([A-Z])([a-z]+)/);
    return m ? [src.replace(new RegExp(`\\b${m[1]}${m[2]}${m[3]}\\b`, "g"), `${m[1]}_${m[2].toLowerCase()}${m[3]}`)] : [];
  },
  "function-style-method": (src) => {
    const m = src.match(/\b([a-z]\w*)\.(map|filter|sum|len)\(([^()]*)\)/);
    if (!m) return [];
    const mod = m[2] === "len" || m[2] === "sum" || m[2] === "map" || m[2] === "filter" ? "List" : "";
    const args = m[3] ? `${m[1]}, ${m[3]}` : m[1];
    return [src.replace(m[0], `${mod}.${m[2]}(${args})`)];
  },
  "mix-logic": (src) => {
    const m = src.match(/(\S+ [<>=!]=? \S+) && (\S+ [<>=!]=? \S+)/);
    return m ? [src.replace(m[0], `${m[0]} || ${m[2]}`)] : [];
  },
  "shadow-binding": (src) => {
    const m = src.match(/^(\s+)let (\w+) = (.*)$/m);
    return m ? [src.replace(m[0], `${m[0]}\n${m[1]}let ${m[2]} = ${m[3]}`)] : [];
  },
};

const PER_OPERATOR = 3; // keeps one operator (match arms) from dominating the set

/** @returns {{operator: string, broken: string, diagnostics: object[]}[]} only mutations the compiler rejects */
export function tracesFor(program, { verify = true, max = Infinity } = {}) {
  const out = [];
  for (const [operator, op] of Object.entries(operators)) {
    let kept = 0;
    for (const broken of op(program)) {
      if (kept >= PER_OPERATOR) break;
      if (broken === program) continue;
      const diagnostics = diagnose(broken, { verify });
      if (diagnostics.length === 0) continue; // the mutation happened to be harmless
      if (diagnostics.some((d) => d.code === "E_INTERNAL")) continue;
      out.push({ operator, broken, diagnostics });
      kept++;
      if (out.length >= max) return out;
    }
  }
  return out;
}
