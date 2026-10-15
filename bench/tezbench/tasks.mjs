// TezBench tasks. Every task has a reference solution in Tez and in TypeScript and a list of cases
// `[call, expected]`; the literals are written so that they mean the same in both languages
// (ints, strings, booleans, lists). These tasks must never appear in training data (see ../README.md).
//
// A candidate is judged on four levels: parses, type-checks, verifies (Tez only), passes all cases.

export const tasks = [
  {
    id: "01-sum-to",
    prompt: "Return the sum of the integers from 1 to n. For n = 0 the sum is 0.",
    tezSig: "fn sumTo(n: Int) -> Int\n  requires n >= 0",
    tsSig: "function sumTo(n: number): number",
    tez: "fn sumTo(n: Int) -> Int\n  requires n >= 0\n  decreases n\n  = if n == 0 { 0 } else { n + sumTo(n - 1) }",
    ts: "function sumTo(n: number): number {\n  return n === 0 ? 0 : n + sumTo(n - 1);\n}",
    cases: [["sumTo(0)", "0"], ["sumTo(1)", "1"], ["sumTo(10)", "55"], ["sumTo(100)", "5050"]],
  },
  {
    id: "02-abs-diff",
    prompt: "Return the absolute difference between a and b.",
    tezSig: "fn absDiff(a: Int, b: Int) -> Int\n  ensures result >= 0",
    tsSig: "function absDiff(a: number, b: number): number",
    tez: "fn absDiff(a: Int, b: Int) -> Int\n  ensures result >= 0\n  = if a > b { a - b } else { b - a }",
    ts: "function absDiff(a: number, b: number): number {\n  return a > b ? a - b : b - a;\n}",
    cases: [["absDiff(3, 10)", "7"], ["absDiff(10, 3)", "7"], ["absDiff(-4, 4)", "8"], ["absDiff(5, 5)", "0"]],
  },
  {
    id: "03-largest",
    prompt: "Return the largest element of a list that is not empty.",
    tezSig: "fn largest(xs: List[Int]) -> Int\n  requires xs.len() > 0",
    tsSig: "function largest(xs: number[]): number",
    tez: "fn largest(xs: List[Int]) -> Int\n  requires xs.len() > 0\n  = xs.fold(xs.get(0), (best, x) => best.max(x))",
    ts: "function largest(xs: number[]): number {\n  return xs.reduce((best, x) => Math.max(best, x), xs[0]);\n}",
    cases: [["largest([3, 9, 2])", "9"], ["largest([-5, -2, -9])", "-2"], ["largest([7])", "7"]],
  },
  {
    id: "04-count-positive",
    prompt: "Count how many elements of the list are greater than zero.",
    tezSig: "fn countPositive(xs: List[Int]) -> Int",
    tsSig: "function countPositive(xs: number[]): number",
    tez: "fn countPositive(xs: List[Int]) -> Int = xs.count(x => x > 0)",
    ts: "function countPositive(xs: number[]): number {\n  return xs.filter((x) => x > 0).length;\n}",
    cases: [["countPositive([1, -2, 3, 0])", "2"], ["countPositive([])", "0"], ["countPositive([-1, -1])", "0"]],
  },
  {
    id: "05-reverse-text",
    prompt: "Return the characters of the text in reverse order.",
    tezSig: "fn reverseText(s: Str) -> Str",
    tsSig: "function reverseText(s: string): string",
    tez: 'fn reverseText(s: Str) -> Str = s.chars().reverse().join("")',
    ts: 'function reverseText(s: string): string {\n  return [...s].reverse().join("");\n}',
    cases: [['reverseText("tez")', '"zet"'], ['reverseText("")', '""'], ['reverseText("a")', '"a"']],
  },
  {
    id: "06-clamp-score",
    prompt: "Limit a score to the range 0 to 100: values below 0 become 0, values above 100 become 100.",
    tezSig: "fn clampScore(x: Int) -> Int\n  ensures 0 <= result && result <= 100",
    tsSig: "function clampScore(x: number): number",
    tez: "fn clampScore(x: Int) -> Int\n  ensures 0 <= result && result <= 100\n  = if x < 0 { 0 } else if x > 100 { 100 } else { x }",
    ts: "function clampScore(x: number): number {\n  return Math.min(100, Math.max(0, x));\n}",
    cases: [["clampScore(-5)", "0"], ["clampScore(50)", "50"], ["clampScore(150)", "100"], ["clampScore(100)", "100"]],
  },
  {
    id: "07-factorial",
    prompt: "Return n factorial (the product of 1 to n). Factorial of 0 is 1. Results can be large.",
    tezSig: "fn factorial(n: Int) -> Int\n  requires n >= 0",
    tsSig: "function factorial(n: number): bigint",
    tez: "fn factorial(n: Int) -> Int\n  requires n >= 0\n  decreases n\n  = if n == 0 { 1 } else { n * factorial(n - 1) }",
    ts: "function factorial(n: number): bigint {\n  return n === 0 ? 1n : BigInt(n) * factorial(n - 1);\n}",
    cases: [["factorial(0)", "1"], ["factorial(5)", "120"], ["factorial(20)", "2432902008176640000"]],
    tsExpect: { 1: "1n", 120: "120n" },
  },
  {
    id: "08-fibonacci",
    prompt: "Return the n-th Fibonacci number, where fib(0) = 0 and fib(1) = 1.",
    tezSig: "fn fib(n: Int) -> Int\n  requires n >= 0",
    tsSig: "function fib(n: number): number",
    tez: "fn fib(n: Int) -> Int\n  requires n >= 0\n  decreases n\n  = if n < 2 { n } else { fib(n - 1) + fib(n - 2) }",
    ts: "function fib(n: number): number {\n  return n < 2 ? n : fib(n - 1) + fib(n - 2);\n}",
    cases: [["fib(0)", "0"], ["fib(1)", "1"], ["fib(10)", "55"], ["fib(15)", "610"]],
  },
  {
    id: "09-is-prime",
    prompt: "Return true when n is a prime number (greater than 1 and divisible only by 1 and itself).",
    tezSig: "fn isPrime(n: Int) -> Bool",
    tsSig: "function isPrime(n: number): boolean",
    tez: "fn isPrime(n: Int) -> Bool = n >= 2 && List.range(2, n).all(d => n % d != 0)",
    ts: "function isPrime(n: number): boolean {\n  if (n < 2) return false;\n  for (let d = 2; d < n; d++) if (n % d === 0) return false;\n  return true;\n}",
    cases: [["isPrime(2)", "true"], ["isPrime(1)", "false"], ["isPrime(15)", "false"], ["isPrime(17)", "true"], ["isPrime(0)", "false"]],
  },
  {
    id: "10-sum-of-squares",
    prompt: "Return the sum of the squares of the elements.",
    tezSig: "fn sumOfSquares(xs: List[Int]) -> Int",
    tsSig: "function sumOfSquares(xs: number[]): number",
    tez: "fn sumOfSquares(xs: List[Int]) -> Int = xs.map(x => x * x).sum()",
    ts: "function sumOfSquares(xs: number[]): number {\n  return xs.reduce((s, x) => s + x * x, 0);\n}",
    cases: [["sumOfSquares([1, 2, 3])", "14"], ["sumOfSquares([])", "0"], ["sumOfSquares([-4])", "16"]],
  },
  {
    id: "11-keep-evens",
    prompt: "Return only the even elements, in their original order.",
    tezSig: "fn keepEvens(xs: List[Int]) -> List[Int]\n  ensures result.len() <= xs.len()",
    tsSig: "function keepEvens(xs: number[]): number[]",
    tez: "fn keepEvens(xs: List[Int]) -> List[Int]\n  ensures result.len() <= xs.len()\n  = xs.filter(x => x % 2 == 0)",
    ts: "function keepEvens(xs: number[]): number[] {\n  return xs.filter((x) => x % 2 === 0);\n}",
    cases: [["keepEvens([1, 2, 3, 4, 6])", "[2, 4, 6]"], ["keepEvens([1, 3])", "[]"], ["keepEvens([])", "[]"]],
  },
  {
    id: "12-double-all",
    prompt: "Return a list with every element multiplied by two.",
    tezSig: "fn doubleAll(xs: List[Int]) -> List[Int]\n  ensures result.len() == xs.len()",
    tsSig: "function doubleAll(xs: number[]): number[]",
    tez: "fn doubleAll(xs: List[Int]) -> List[Int]\n  ensures result.len() == xs.len()\n  = xs.map(x => x * 2)",
    ts: "function doubleAll(xs: number[]): number[] {\n  return xs.map((x) => x * 2);\n}",
    cases: [["doubleAll([1, 2, 3])", "[2, 4, 6]"], ["doubleAll([])", "[]"], ["doubleAll([-1])", "[-2]"]],
  },
  {
    id: "13-join-words",
    prompt: "Join the words with a comma and a space between them.",
    tezSig: "fn joinWords(ws: List[Str]) -> Str",
    tsSig: "function joinWords(ws: string[]): string",
    tez: 'fn joinWords(ws: List[Str]) -> Str = ws.join(", ")',
    ts: 'function joinWords(ws: string[]): string {\n  return ws.join(", ");\n}',
    cases: [['joinWords(["a", "b", "c"])', '"a, b, c"'], ['joinWords(["x"])', '"x"'], ["joinWords([])", '""']],
  },
  {
    id: "14-word-count",
    prompt: "Count the words in a text. Words are separated by single spaces; the empty text has 0 words.",
    tezSig: "fn wordCount(s: Str) -> Int",
    tsSig: "function wordCount(s: string): number",
    tez: 'fn wordCount(s: Str) -> Int = s.split(" ").filter(w => w.len() > 0).len()',
    ts: 'function wordCount(s: string): number {\n  return s.split(" ").filter((w) => w.length > 0).length;\n}',
    cases: [['wordCount("one two three")', "3"], ['wordCount("")', "0"], ['wordCount("solo")', "1"]],
  },
  {
    id: "15-capitalize",
    prompt: "Return the text with its first character in upper case and the rest unchanged. The text is not empty.",
    tezSig: "fn capitalize(s: Str) -> Str\n  requires s.len() > 0",
    tsSig: "function capitalize(s: string): string",
    tez: "fn capitalize(s: Str) -> Str\n  requires s.len() > 0\n  = s.slice(0, 1).upper() ++ s.slice(1, s.len())",
    ts: "function capitalize(s: string): string {\n  return s.slice(0, 1).toUpperCase() + s.slice(1);\n}",
    cases: [['capitalize("tez")', '"Tez"'], ['capitalize("a")', '"A"'], ['capitalize("Hello")', '"Hello"']],
  },
  {
    id: "16-average",
    prompt: "Return the average of a list that is not empty, rounded toward zero (integer division).",
    tezSig: "fn average(xs: List[Int]) -> Int\n  requires xs.len() > 0",
    tsSig: "function average(xs: number[]): number",
    tez: "fn average(xs: List[Int]) -> Int\n  requires xs.len() > 0\n  = xs.sum() / xs.len()",
    ts: "function average(xs: number[]): number {\n  return Math.trunc(xs.reduce((a, b) => a + b, 0) / xs.length);\n}",
    cases: [["average([2, 4, 6])", "4"], ["average([1, 2])", "1"], ["average([-7, -8])", "-7"], ["average([9])", "9"]],
  },
  {
    id: "17-last-or",
    prompt: "Return the last element of the list, or the fallback when the list is empty.",
    tezSig: "fn lastOr(xs: List[Int], fallback: Int) -> Int",
    tsSig: "function lastOr(xs: number[], fallback: number): number",
    tez: "fn lastOr(xs: List[Int], fallback: Int) -> Int = xs.last().unwrapOr(fallback)",
    ts: "function lastOr(xs: number[], fallback: number): number {\n  return xs.length > 0 ? xs[xs.length - 1] : fallback;\n}",
    cases: [["lastOr([1, 2, 3], 0)", "3"], ["lastOr([], 42)", "42"], ["lastOr([5], 0)", "5"]],
  },
  {
    id: "18-gcd",
    prompt: "Return the greatest common divisor of two non-negative integers (Euclid's algorithm). gcd(a, 0) is a.",
    tezSig: "fn gcd(a: Int, b: Int) -> Int\n  requires a >= 0 && b >= 0",
    tsSig: "function gcd(a: number, b: number): number",
    tez: "fn gcd(a: Int, b: Int) -> Int\n  requires a >= 0 && b >= 0\n  decreases b\n  = if b == 0 { a } else { gcd(b, a % b) }",
    ts: "function gcd(a: number, b: number): number {\n  return b === 0 ? a : gcd(b, a % b);\n}",
    cases: [["gcd(84, 36)", "12"], ["gcd(7, 0)", "7"], ["gcd(17, 5)", "1"], ["gcd(0, 9)", "9"]],
  },
  {
    id: "19-power",
    prompt: "Return base raised to the power exp, for exp >= 0. Results can be large.",
    tezSig: "fn power(base: Int, exp: Int) -> Int\n  requires exp >= 0",
    tsSig: "function power(base: bigint, exp: number): bigint",
    tez: "fn power(base: Int, exp: Int) -> Int\n  requires exp >= 0\n  decreases exp\n  = if exp == 0 { 1 } else { base * power(base, exp - 1) }",
    ts: "function power(base: bigint, exp: number): bigint {\n  return exp === 0 ? 1n : base * power(base, exp - 1);\n}",
    cases: [["power(2, 10)", "1024"], ["power(5, 0)", "1"], ["power(3, 4)", "81"], ["power(-2, 3)", "-8"]],
    tsBigint: true,
  },
  {
    id: "20-zip-sum",
    prompt: "Add two lists element by element. If the lists differ in length, stop at the shorter one.",
    tezSig: "fn zipSum(a: List[Int], b: List[Int]) -> List[Int]",
    tsSig: "function zipSum(a: number[], b: number[]): number[]",
    tez: "fn zipSum(a: List[Int], b: List[Int]) -> List[Int] = a.zip(b).map(p => p.0 + p.1)",
    ts: "function zipSum(a: number[], b: number[]): number[] {\n  return a.slice(0, Math.min(a.length, b.length)).map((x, i) => x + b[i]);\n}",
    cases: [["zipSum([1, 2, 3], [10, 20, 30])", "[11, 22, 33]"], ["zipSum([1, 2], [5])", "[6]"], ["zipSum([], [1])", "[]"]],
  },
];

/** Tez source of a task's tests, appended to a candidate. */
export function tezTests(task) {
  return task.cases.map(([call, want], i) => `test case${i + 1} = ${call} == ${want}`).join("\n");
}

/** TypeScript harness: runs the cases after a candidate. Bigint-returning tasks compare as bigint. */
export function tsTests(task) {
  const big = task.id === "07-factorial" || task.tsBigint;
  const lit = (s) => (big && /^-?\d+$/.test(s) ? `${s}n` : s);
  const callBig = (c) => (task.tsBigint ? c.replace(/\((-?\d+),/, "($1n,") : c);
  const lines = task.cases.map(([call, want], i) => `check(${i + 1}, () => ${callBig(call)}, ${lit(want)});`);
  return [
    "let failed = 0;",
    "const eq = (a: unknown, b: unknown) => JSON.stringify(a, (_k, v) => (typeof v === 'bigint' ? v.toString() : v)) === JSON.stringify(b, (_k, v) => (typeof v === 'bigint' ? v.toString() : v));",
    "function check(n: number, f: () => unknown, want: unknown) {",
    "  let got: unknown;",
    "  try { got = f(); } catch { got = 'threw'; }",
    "  if (!eq(got, want)) { failed++; console.log(`FAILED case${n}`); }",
    "}",
    ...lines,
    "console.log(`${failed === 0 ? 'ok' : 'failed'}: ${" + task.cases.length + " - failed} of " + task.cases.length + "`);",
    "process.exit(failed === 0 ? 0 : 1);",
  ].join("\n");
}

/** The prompt shown to a model. `card` is the language card text (or empty for the no-card baseline). */
export function prompt(task, lang, card = "") {
  const sig = lang === "tez" ? task.tezSig : task.tsSig;
  const head =
    lang === "tez"
      ? "Write the following function in Tez. Reply with Tez code only, no explanation.\n\n" + (card ? `${card}\n\n` : "")
      : "Write the following function in TypeScript. Reply with TypeScript code only, no explanation.\n\n";
  return `${head}Task: ${task.prompt}\n\nSignature:\n${sig}\n`;
}
