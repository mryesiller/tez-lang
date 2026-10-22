// Ways to break an accepted program, so that the model can learn to read the compiler's diagnostics and repair the
// program. `operators` come from ../mutate.mjs (match arms, contracts, naming, ...); the app operators add the mistakes
// a model makes when it writes a web page.
import { operators } from "../mutate.mjs";

const first = (src, re) => {
  const m = src.match(re);
  return m ? m : null;
};

const appOnly = {
  "unknown-html-function": (src) => {
    const swaps = [["Html.button(", "Html.buton("], ["Html.paragraph(", "Html.para("], ["Html.heading(", "Html.title("], ["Html.row(", "Html.line("], ["Html.input(", "Html.textbox("]];
    return swaps.filter(([from]) => src.includes(from)).map(([from, to]) => src.replace(from, to));
  },
  "button-without-label": (src) => {
    const m = first(src, /Html\.button\((?:"[^"]*"|[^,()]+(?:\([^()]*\))?|\([^()]*\)), /);
    return m ? [src.replace(m[0], "Html.button(")] : [];
  },
  "unknown-message": (src) => {
    const m = first(src, /Html\.button\([^\n]*?, ([A-Z][A-Za-z]*)\)/);
    return m ? [src.replace(m[0], m[0].replace(new RegExp(`${m[1]}\\)$`), `${m[1]}Now)`))] : [];
  },
  "number-as-text": (src) => {
    const i = src.indexOf(".toStr()");
    return i >= 0 && /\+\+/.test(src.slice(Math.max(0, i - 80), i)) ? [src.slice(0, i) + src.slice(i + ".toStr()".length)] : [];
  },
  "drop-guard": (src) => {
    const m = first(src, /if ([^{}\n]+?) \{ (m with \{[^{}]*\}|Model \{[^{}]*\}) \} else/);
    return m && /where it/.test(src) ? [src.replace(`if ${m[1]} {`, "if true {")] : [];
  },
  "missing-field": (src) => {
    const m = first(src, /Model \{ ([a-z]+): [^,{}]+, ([a-z]+): [^{}]+ \}/);
    return m ? [src.replace(m[0], m[0].replace(/, [a-z]+: [^{}]+ \}$/, " }"))] : [];
  },
  "wrong-return-type": (src) => {
    const m = first(src, /fn view\(m: Model\) -> Html\[Msg\]/);
    return m ? [src.replace(m[0], "fn view(m: Model) -> Str")] : [];
  },
  "forgotten-subscription-type": (src) => {
    const m = first(src, /Sub\.every\((\d+), /);
    return m ? [src.replace(m[0], `Sub.every(${Math.min(40, Number(m[1]))}, `)] : [];
  },
};

// Mistakes the prover catches: the program parses and type-checks, but a proof obligation fails. Learning to read
// these diagnostics (with their counterexamples) is the heart of the repair loop.
const proofOps = {
  "off-by-one-guard": (src) => {
    const out = [];
    for (const m of src.matchAll(/(if [^{}\n]*?)([<>])( -?\d+| [a-z][A-Za-z.]*)( \{)/g)) {
      if (/^test /m.test(src.slice(src.lastIndexOf("\n", m.index) + 1, m.index + 5))) continue;
      out.push(src.slice(0, m.index) + `${m[1]}${m[2]}=${m[3]}${m[4]}` + src.slice(m.index + m[0].length));
    }
    return /where it|requires|ensures|\/ |% /.test(src) ? out.slice(0, 3) : [];
  },
  "wrong-zero-check": (src) => {
    const m = first(src, /if ([a-z][A-Za-z]*) == 0 \{/);
    return m && /[/%] /.test(src) ? [src.replace(m[0], `if ${m[1]} == 1 {`)] : [];
  },
  "constant-decreases": (src) => {
    const m = first(src, /decreases [^\n]+/);
    return m ? [src.replace(m[0], "decreases 0")] : [];
  },
  "weaken-requires": (src) => {
    const lines = src.split("\n");
    const out = [];
    lines.forEach((l, i) => {
      if (!/^\s+requires\b/.test(l)) return;
      const w = l.replace(/>= (\d+)/, (_, n) => `>= ${Number(n) - 1}`).replace(/ > (\d+)/, (_, n) => ` > ${Number(n) - 1}`).replace(/ <= ([a-z]+)\b/, " <= $1 + 1");
      if (w !== l) out.push(lines.map((x, j) => (j === i ? w : x)).join("\n"));
    });
    return out.slice(0, 2);
  },
  "strengthen-ensures": (src) => {
    const m = first(src, /ensures result (>=|<=) (-?\d+)/);
    return m ? [src.replace(m[0], `ensures result ${m[1] === ">=" ? ">" : "<"} ${m[2]}`)] : [];
  },
};
export const PROOF_OPERATORS = new Set(Object.keys(proofOps).concat(["drop-requires", "drop-decreases", "drop-ensures-weaken", "drop-guard", "drop-match-arm"]));

export const pureOperators = { ...operators, ...proofOps };
export const appOperators = { ...operators, ...proofOps, ...appOnly };
