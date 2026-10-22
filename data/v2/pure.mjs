// Task families for pure functions. A family is `(r) => task` where `r` is a seeded random source and the task is
//   { name, desc, sig, code, cases: [[tez expression, tez literal]], types? }
// `sig` is what a caller is told (the signature with its contract lines), `code` is a complete correct solution and `cases`
// are checked by the compiler (`tez test`) against the solution: a task whose cases fail is never written out.
// Several families are deliberately NOT the TezBench tasks (sum to n, abs diff, largest, count positive, reverse text,
// clamp score, factorial, fibonacci, primality, sum of squares, keep evens, double all, join words, word count,
// capitalize, average, last-or, gcd, power, zip sum): those are held out for evaluation.
import { idiv, imod, lit, raw, tup } from "./rng.mjs";

export const families = [];
const fam = (id, make) => families.push({ id, make });

const NAMES = {
  int: ["n", "x", "value", "amount", "count"],
  list: ["xs", "numbers", "values", "items", "scores"],
};
const nm = (r, k) => r.pick(NAMES[k]);

// ---------------------------------------------------------------------------------------------- integers
fam("int.add-const", (r) => {
  const k = r.int(3, 99);
  const name = r.pick([`plus${k}`, `add${k}`, `shiftBy${k}`, `raiseBy${k}`]);
  const a = nm(r, "int");
  return {
    name,
    desc: `Return ${a} plus ${k}.`,
    sig: `fn ${name}(${a}: Int) -> Int`,
    code: `fn ${name}(${a}: Int) -> Int = ${a} + ${k}`,
    cases: [-5, 0, 7, 100].map((v) => [`${name}(${lit(v)})`, lit(v + k)]),
  };
});

fam("int.scale-const", (r) => {
  const k = r.pick([3, 4, 5, 6, 7, 8, 9, 10, 12, 15, 20, 25, 100]);
  const name = r.pick([`times${k}`, `scaleBy${k}`, `multiplyBy${k}`]);
  const a = nm(r, "int");
  return {
    name,
    desc: `Return ${a} multiplied by ${k}.`,
    sig: `fn ${name}(${a}: Int) -> Int`,
    code: `fn ${name}(${a}: Int) -> Int = ${a} * ${k}`,
    cases: [-3, 0, 1, 12].map((v) => [`${name}(${lit(v)})`, lit(v * k)]),
  };
});

fam("int.affine", (r) => {
  const a = r.int(2, 9);
  const b = r.int(1, 30);
  const name = r.pick(["price", "score", "fee", "cost", "total", "points"]);
  return {
    name,
    desc: `The ${name} is ${a} times the quantity plus a fixed amount of ${b}. Return it for a given quantity.`,
    sig: `fn ${name}(quantity: Int) -> Int`,
    code: `fn ${name}(quantity: Int) -> Int = ${a} * quantity + ${b}`,
    cases: [0, 1, 5, 40].map((v) => [`${name}(${v})`, lit(a * v + b)]),
  };
});

fam("int.divisible-by", (r) => {
  const k = r.pick([3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15]);
  const name = r.pick([`divisibleBy${k}`, `isMultipleOf${k}`, `hasFactor${k}`]);
  return {
    name,
    desc: `Return true when n is a multiple of ${k}.`,
    sig: `fn ${name}(n: Int) -> Bool`,
    code: `fn ${name}(n: Int) -> Bool = n % ${k} == 0`,
    cases: [0, k, k * 3, k + 1, -k].map((v) => [`${name}(${lit(v)})`, lit(imod(v, k) === 0)]),
  };
});

fam("int.median-of-three", (r) => {
  const name = r.pick(["middleOfThree", "median3", "midValue"]);
  const med = (a, b, c) => [a, b, c].sort((x, y) => x - y)[1];
  return {
    name,
    desc: "Return the value that is neither the smallest nor the largest of the three integers (the median).",
    sig: `fn ${name}(a: Int, b: Int, c: Int) -> Int`,
    code: `fn ${name}(a: Int, b: Int, c: Int) -> Int = a.min(b).max(a.max(b).min(c))`,
    cases: [[1, 2, 3], [3, 1, 2], [9, 9, 1], [-4, 7, 0], [5, 5, 5]].map(([a, b, c]) => [`${name}(${a}, ${b}, ${c})`, lit(med(a, b, c))]),
  };
});

fam("int.power-fixed", (r) => {
  const e = r.pick([3, 4]);
  const name = e === 3 ? r.pick(["cube", "cubed"]) : "fourthPower";
  const body = Array(e).fill("n").join(" * ");
  return {
    name,
    desc: `Return n raised to the power ${e}.`,
    sig: `fn ${name}(n: Int) -> Int`,
    code: `fn ${name}(n: Int) -> Int = ${body}`,
    cases: [-2, 0, 1, 3, 5].map((v) => [`${name}(${lit(v)})`, lit(v ** e)]),
  };
});

fam("int.midpoint", (r) => {
  const name = r.pick(["midpoint", "halfway", "meanOfTwo"]);
  return {
    name,
    desc: "Return the integer halfway between a and b, rounding toward zero: (a + b) divided by 2.",
    sig: `fn ${name}(a: Int, b: Int) -> Int`,
    code: `fn ${name}(a: Int, b: Int) -> Int = (a + b) / 2`,
    cases: [[2, 4], [1, 2], [-3, 8], [0, 0], [-7, -2]].map(([a, b]) => [`${name}(${a}, ${b})`, lit(idiv(a + b, 2))]),
  };
});

fam("int.percent-of", (r) => {
  const name = r.pick(["percentOf", "shareInPercent", "ratioPercent"]);
  return {
    name,
    desc: "Return what percent part is of total, rounded down. The total is positive.",
    sig: `fn ${name}(part: Int, total: Int) -> Int\n  requires total > 0`,
    code: `fn ${name}(part: Int, total: Int) -> Int\n  requires total > 0\n  = part * 100 / total`,
    cases: [[1, 4], [50, 200], [3, 7], [0, 5]].map(([p, t]) => [`${name}(${p}, ${t})`, lit(idiv(p * 100, t))]),
  };
});

fam("int.div-ceil", (r) => {
  const name = r.pick(["divCeil", "pagesNeeded", "groupsNeeded", "boxesNeeded"]);
  const [x, y] = name === "divCeil" ? ["a", "b"] : ["items", "perGroup"];
  return {
    name,
    desc: `Return ${x} divided by ${y}, rounded up. ${x} is not negative and ${y} is positive.`,
    sig: `fn ${name}(${x}: Int, ${y}: Int) -> Int\n  requires ${x} >= 0 && ${y} > 0`,
    code: `fn ${name}(${x}: Int, ${y}: Int) -> Int\n  requires ${x} >= 0 && ${y} > 0\n  = (${x} + ${y} - 1) / ${y}`,
    cases: [[10, 3], [9, 3], [0, 4], [1, 5], [100, 7]].map(([a, b]) => [`${name}(${a}, ${b})`, lit(Math.ceil(a / b))]),
  };
});

fam("int.saturating-sub", (r) => {
  const name = r.pick(["saturatingSub", "remainingAfter", "stockLeft", "budgetLeft"]);
  return {
    name,
    desc: "Return a minus b, but never less than zero.",
    sig: `fn ${name}(a: Int, b: Int) -> Int\n  ensures result >= 0`,
    code: `fn ${name}(a: Int, b: Int) -> Int\n  ensures result >= 0\n  = if a > b { a - b } else { 0 }`,
    cases: [[10, 3], [3, 10], [5, 5], [0, 0]].map(([a, b]) => [`${name}(${a}, ${b})`, lit(Math.max(a - b, 0))]),
  };
});

fam("int.digit-sum", (r) => {
  const name = r.pick(["digitSum", "sumOfDigits", "digitTotal"]);
  const ds = (n) => String(n).split("").reduce((s, c) => s + Number(c), 0);
  return {
    name,
    desc: "Return the sum of the decimal digits of n (n is not negative).",
    sig: `fn ${name}(n: Int) -> Int\n  requires n >= 0`,
    code: `fn ${name}(n: Int) -> Int\n  requires n >= 0\n  decreases n\n  = if n < 10 { n } else { n % 10 + ${name}(n / 10) }`,
    cases: [0, 7, 10, 123, 9999, 100200].map((v) => [`${name}(${v})`, lit(ds(v))]),
  };
});

fam("int.count-digits", (r) => {
  const name = r.pick(["countDigits", "digitCount", "numberLength"]);
  return {
    name,
    desc: "Return how many decimal digits n has (0 has one digit; n is not negative).",
    sig: `fn ${name}(n: Int) -> Int\n  requires n >= 0`,
    code: `fn ${name}(n: Int) -> Int\n  requires n >= 0\n  decreases n\n  = if n < 10 { 1 } else { 1 + ${name}(n / 10) }`,
    cases: [0, 9, 10, 99, 100, 123456].map((v) => [`${name}(${v})`, lit(String(v).length)]),
  };
});

fam("int.countdown-list", (r) => {
  const name = r.pick(["countDown", "downFrom", "descending"]);
  return {
    name,
    desc: "Return the list n, n-1, ..., 1 (empty when n is 0). n is not negative.",
    sig: `fn ${name}(n: Int) -> List[Int]\n  requires n >= 0`,
    code: `fn ${name}(n: Int) -> List[Int]\n  requires n >= 0\n  decreases n\n  = if n == 0 { [] } else { [n].concat(${name}(n - 1)) }`,
    cases: [0, 1, 4, 6].map((v) => [`${name}(${v})`, lit(Array.from({ length: v }, (_, i) => v - i))]),
  };
});

fam("int.leap-year", (r) => {
  const name = r.pick(["isLeapYear", "leapYear"]);
  const leap = (y) => y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);
  return {
    name,
    desc: "Return true when the year is a leap year: divisible by 4, except years divisible by 100 that are not divisible by 400.",
    sig: `fn ${name}(year: Int) -> Bool`,
    code: `fn ${name}(year: Int) -> Bool = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)`,
    cases: [1996, 1900, 2000, 2023, 2024, 2100].map((y) => [`${name}(${y})`, lit(leap(y))]),
  };
});

fam("int.age-group", (r) => {
  const a = r.int(10, 14);
  const b = r.int(18, 21);
  const c = r.int(60, 67);
  const name = r.pick(["ageGroup", "lifeStage", "category"]);
  const f = (n) => (n < a ? "child" : n < b ? "teen" : n < c ? "adult" : "senior");
  return {
    name,
    desc: `Return "child" for ages below ${a}, "teen" below ${b}, "adult" below ${c} and "senior" otherwise.`,
    sig: `fn ${name}(age: Int) -> Str`,
    code: `fn ${name}(age: Int) -> Str =\n  if age < ${a} { "child" } else if age < ${b} { "teen" } else if age < ${c} { "adult" } else { "senior" }`,
    cases: [0, a - 1, a, b - 1, b, c - 1, c, 90].map((v) => [`${name}(${v})`, lit(f(v))]),
  };
});

fam("int.letter-grade", (r) => {
  const t = [r.int(88, 92), r.int(78, 82), r.int(68, 72), r.int(58, 62)];
  const name = r.pick(["letterGrade", "gradeFor", "markFor"]);
  const f = (s) => (s >= t[0] ? "A" : s >= t[1] ? "B" : s >= t[2] ? "C" : s >= t[3] ? "D" : "F");
  return {
    name,
    desc: `Return "A" for a score of at least ${t[0]}, "B" for at least ${t[1]}, "C" for at least ${t[2]}, "D" for at least ${t[3]} and "F" below that.`,
    sig: `fn ${name}(score: Int) -> Str`,
    code: `fn ${name}(score: Int) -> Str =\n  if score >= ${t[0]} { "A" } else if score >= ${t[1]} { "B" } else if score >= ${t[2]} { "C" } else if score >= ${t[3]} { "D" } else { "F" }`,
    cases: [100, t[0], t[0] - 1, t[1], t[2], t[3], t[3] - 1, 0].map((v) => [`${name}(${v})`, lit(f(v))]),
  };
});

fam("int.sign", (r) => {
  const name = r.pick(["sign", "signOf", "direction"]);
  return {
    name,
    desc: "Return -1 for a negative number, 1 for a positive number and 0 for zero.",
    sig: `fn ${name}(n: Int) -> Int\n  ensures result >= -1 && result <= 1`,
    code: `fn ${name}(n: Int) -> Int\n  ensures result >= -1 && result <= 1\n  = if n < 0 { -1 } else if n > 0 { 1 } else { 0 }`,
    cases: [-9, -1, 0, 1, 44].map((v) => [`${name}(${v})`, lit(Math.sign(v))]),
  };
});

fam("int.between", (r) => {
  const name = r.pick(["isBetween", "inRange", "withinBounds"]);
  return {
    name,
    desc: "Return true when x lies between lo and hi, both ends included.",
    sig: `fn ${name}(x: Int, lo: Int, hi: Int) -> Bool`,
    code: `fn ${name}(x: Int, lo: Int, hi: Int) -> Bool = x >= lo && x <= hi`,
    cases: [[5, 1, 10], [1, 1, 10], [10, 1, 10], [0, 1, 10], [11, 1, 10], [3, 5, 2]].map(([x, lo, hi]) => [`${name}(${x}, ${lo}, ${hi})`, lit(x >= lo && x <= hi)]),
  };
});

fam("int.discount", (r) => {
  const name = r.pick(["discounted", "priceAfterDiscount", "salePrice"]);
  return {
    name,
    desc: "Return the price after taking the given percent off, rounded down. The price is not negative and the percent is between 0 and 100.",
    sig: `fn ${name}(price: Int, percent: Int) -> Int\n  requires price >= 0 && percent >= 0 && percent <= 100`,
    code: `fn ${name}(price: Int, percent: Int) -> Int\n  requires price >= 0 && percent >= 0 && percent <= 100\n  = price * (100 - percent) / 100`,
    cases: [[200, 25], [99, 10], [50, 0], [80, 100], [0, 30]].map(([p, d]) => [`${name}(${p}, ${d})`, lit(idiv(p * (100 - d), 100))]),
  };
});

fam("int.temperature", (r) => {
  const toF = r.chance(0.5);
  const name = toF ? r.pick(["toFahrenheit", "celsiusToF"]) : r.pick(["toCelsius", "fahrenheitToC"]);
  return toF
    ? {
        name,
        desc: "Convert degrees Celsius to degrees Fahrenheit, rounded down: c * 9 / 5 + 32.",
        sig: `fn ${name}(c: Int) -> Int`,
        code: `fn ${name}(c: Int) -> Int = c * 9 / 5 + 32`,
        cases: [0, 100, 37, 20, 10].map((c) => [`${name}(${c})`, lit(idiv(c * 9, 5) + 32)]),
      }
    : {
        name,
        desc: "Convert degrees Fahrenheit to degrees Celsius, rounded toward zero: (f - 32) * 5 / 9.",
        sig: `fn ${name}(f: Int) -> Int`,
        code: `fn ${name}(f: Int) -> Int = (f - 32) * 5 / 9`,
        cases: [32, 212, 98, 50, 14].map((f) => [`${name}(${f})`, lit(idiv((f - 32) * 5, 9))]),
      };
});

fam("int.clock-format", (r) => {
  const name = r.pick(["formatClock", "clockText", "minutesAndSeconds"]);
  const pad = (n) => (n < 10 ? `0${n}` : String(n));
  return {
    name,
    desc: "Format a number of seconds as MM:SS with two digits each (for example 125 becomes \"02:05\"). Minutes may exceed 59; the input is not negative.",
    sig: `fn ${name}(total: Int) -> Str\n  requires total >= 0`,
    code: `fn pad(n: Int) -> Str = if n < 10 { "0" ++ n.toStr() } else { n.toStr() }\n\nfn ${name}(total: Int) -> Str\n  requires total >= 0\n  = pad(total / 60) ++ ":" ++ pad(total % 60)`,
    cases: [0, 5, 60, 125, 3599, 6000].map((t) => [`${name}(${t})`, lit(`${pad(Math.floor(t / 60))}:${pad(t % 60)}`)]),
  };
});

fam("int.ticket-price", (r) => {
  const [kid, adult, senior] = [r.int(3, 8), r.int(10, 20), r.int(6, 12)];
  const name = r.pick(["ticketPrice", "entryFee", "admission"]);
  const f = (a) => (a < 12 ? kid : a >= 65 ? senior : adult);
  return {
    name,
    desc: `A ticket costs ${kid} for under 12, ${senior} for 65 and over, and ${adult} otherwise. Return the price for the age.`,
    sig: `fn ${name}(age: Int) -> Int`,
    code: `fn ${name}(age: Int) -> Int = if age < 12 { ${kid} } else if age >= 65 { ${senior} } else { ${adult} }`,
    cases: [0, 11, 12, 40, 64, 65, 90].map((a) => [`${name}(${a})`, lit(f(a))]),
  };
});

fam("int.xor-implies", (r) => {
  const which = r.pick(["xor", "implies", "nand"]);
  const spec = {
    xor: ["Return true when exactly one of a and b is true.", "a != b", (a, b) => a !== b],
    implies: ["Return false only when a is true and b is false (a implies b).", "!a || b", (a, b) => !a || b],
    nand: ["Return false only when both a and b are true.", "!(a && b)", (a, b) => !(a && b)],
  }[which];
  return {
    name: which,
    desc: spec[0],
    sig: `fn ${which}(a: Bool, b: Bool) -> Bool`,
    code: `fn ${which}(a: Bool, b: Bool) -> Bool = ${spec[1]}`,
    cases: [[true, true], [true, false], [false, true], [false, false]].map(([a, b]) => [`${which}(${a}, ${b})`, lit(spec[2](a, b))]),
  };
});

// ---------------------------------------------------------------------------------------------- lists of integers
const listCases = (r, n = 4, lo = -9, hi = 20) => {
  const out = [[], r.list(1, lo, hi)];
  while (out.length < n) out.push(r.list(r.int(2, 6), lo, hi));
  return out;
};

fam("list.add-to-all", (r) => {
  const k = r.int(3, 50);
  const name = r.pick([`addToAll`, `shiftAll`, `raiseAll`]);
  const xs = nm(r, "list");
  return {
    name,
    desc: `Return the list with ${k} added to every element.`,
    sig: `fn ${name}(${xs}: List[Int]) -> List[Int]`,
    code: `fn ${name}(${xs}: List[Int]) -> List[Int] = ${xs}.map(x => x + ${k})`,
    cases: listCases(r).map((l) => [`${name}(${lit(l)})`, lit(l.map((x) => x + k))]),
  };
});

fam("list.scale-all", (r) => {
  const k = r.pick([3, 4, 5, 7, 10]);
  const name = r.pick([`timesAll`, `scaleAll`, `multiplyAll`]);
  return {
    name,
    desc: `Return the list with every element multiplied by ${k}.`,
    sig: `fn ${name}(xs: List[Int]) -> List[Int]`,
    code: `fn ${name}(xs: List[Int]) -> List[Int] = xs.map(x => x * ${k})`,
    cases: listCases(r).map((l) => [`${name}(${lit(l)})`, lit(l.map((x) => x * k))]),
  };
});

fam("list.negate-or-square", (r) => {
  const sq = r.chance(0.5);
  const name = sq ? r.pick(["squares", "squareEach"]) : r.pick(["negateAll", "flipSigns"]);
  return {
    name,
    desc: sq ? "Return the square of every element, in the same order." : "Return the list with the sign of every element flipped.",
    sig: `fn ${name}(xs: List[Int]) -> List[Int]`,
    code: sq ? `fn ${name}(xs: List[Int]) -> List[Int] = xs.map(x => x * x)` : `fn ${name}(xs: List[Int]) -> List[Int] = xs.map(x => 0 - x)`,
    cases: listCases(r).map((l) => [`${name}(${lit(l)})`, lit(l.map((x) => (sq ? x * x : -x)))]),
  };
});

fam("list.keep-compare", (r) => {
  const k = r.int(-3, 12);
  const kind = r.pick(["above", "below", "atLeast", "notEqual"]);
  const spec = {
    above: [`greater than ${k}`, `x > ${k}`, (x) => x > k, `keepAbove${k < 0 ? "Neg" + -k : k}`],
    below: [`less than ${k}`, `x < ${k}`, (x) => x < k, `keepBelow${k < 0 ? "Neg" + -k : k}`],
    atLeast: [`at least ${k}`, `x >= ${k}`, (x) => x >= k, `keepAtLeast${k < 0 ? "Neg" + -k : k}`],
    notEqual: [`different from ${k}`, `x != ${k}`, (x) => x !== k, `without${k < 0 ? "Neg" + -k : k}`],
  }[kind];
  const name = spec[3];
  return {
    name,
    desc: `Return the elements that are ${spec[0]}, keeping their order.`,
    sig: `fn ${name}(xs: List[Int]) -> List[Int]`,
    code: `fn ${name}(xs: List[Int]) -> List[Int] = xs.filter(x => ${spec[1]})`,
    cases: listCases(r, 5).map((l) => [`${name}(${lit(l)})`, lit(l.filter(spec[2]))]),
  };
});

fam("list.keep-multiples", (r) => {
  const k = r.pick([3, 4, 5, 6, 7, 10]);
  const name = `keepMultiplesOf${k}`;
  return {
    name,
    desc: `Return the elements that are multiples of ${k}, keeping their order.`,
    sig: `fn ${name}(xs: List[Int]) -> List[Int]`,
    code: `fn ${name}(xs: List[Int]) -> List[Int] = xs.filter(x => x % ${k} == 0)`,
    cases: listCases(r, 5, 0, 40).map((l) => [`${name}(${lit(l)})`, lit(l.filter((x) => x % k === 0))]),
  };
});

fam("list.drop-negatives", (r) => {
  const name = r.pick(["onlyNonNegative", "dropNegatives", "withoutNegatives"]);
  return {
    name,
    desc: "Return the elements that are not negative, keeping their order.",
    sig: `fn ${name}(xs: List[Int]) -> List[Int]`,
    code: `fn ${name}(xs: List[Int]) -> List[Int] = xs.filter(x => x >= 0)`,
    cases: listCases(r, 5, -9, 9).map((l) => [`${name}(${lit(l)})`, lit(l.filter((x) => x >= 0))]),
  };
});

fam("list.count-compare", (r) => {
  const k = r.int(1, 15);
  const kind = r.pick(["above", "equal", "multiples", "below"]);
  const spec = {
    above: [`greater than ${k}`, `x > ${k}`, (x) => x > k, `countAbove${k}`],
    equal: [`equal to ${k}`, `x == ${k}`, (x) => x === k, `countOf${k}`],
    multiples: [`multiples of ${k}`, `x % ${k} == 0`, (x) => x % k === 0, `countMultiplesOf${k}`],
    below: [`less than ${k}`, `x < ${k}`, (x) => x < k, `countBelow${k}`],
  }[kind];
  const name = spec[3];
  return {
    name,
    desc: `Return how many elements are ${spec[0]}.`,
    sig: `fn ${name}(xs: List[Int]) -> Int`,
    code: `fn ${name}(xs: List[Int]) -> Int = xs.count(x => ${spec[1]})`,
    cases: listCases(r, 5, 0, 30).map((l) => [`${name}(${lit(l)})`, lit(l.filter(spec[2]).length)]),
  };
});

fam("list.sum-variants", (r) => {
  const kind = r.pick(["cubes", "abs", "above", "doubledPlusOne", "pairsDifference"]);
  const k = r.int(2, 9);
  const spec = {
    cubes: ["sumOfCubes", "Return the sum of the cubes of the elements.", "xs.map(x => x * x * x).sum()", (l) => l.reduce((s, x) => s + x ** 3, 0)],
    abs: ["sumOfAbs", "Return the sum of the absolute values of the elements.", "xs.map(x => x.abs()).sum()", (l) => l.reduce((s, x) => s + Math.abs(x), 0)],
    above: [`sumAbove${k}`, `Return the sum of the elements greater than ${k}.`, `xs.filter(x => x > ${k}).sum()`, (l) => l.filter((x) => x > k).reduce((s, x) => s + x, 0)],
    doubledPlusOne: ["totalPlusOnes", "Return the sum of the elements, with one added to each element first.", "xs.map(x => x + 1).sum()", (l) => l.reduce((s, x) => s + x + 1, 0)],
    pairsDifference: ["spread", "Return the difference between the sum of the elements above 5 and the sum of the elements below 5.", "xs.filter(x => x > 5).sum() - xs.filter(x => x < 5).sum()", (l) => l.filter((x) => x > 5).reduce((s, x) => s + x, 0) - l.filter((x) => x < 5).reduce((s, x) => s + x, 0)],
  }[kind];
  return {
    name: spec[0],
    desc: spec[1],
    sig: `fn ${spec[0]}(xs: List[Int]) -> Int`,
    code: `fn ${spec[0]}(xs: List[Int]) -> Int = ${spec[2]}`,
    cases: listCases(r, 5, -6, 14).map((l) => [`${spec[0]}(${lit(l)})`, lit(spec[3](l))]),
  };
});

fam("list.product", (r) => {
  const name = r.pick(["product", "productOf", "multiplyAllTogether"]);
  return {
    name,
    desc: "Return the product of all elements (1 for an empty list).",
    sig: `fn ${name}(xs: List[Int]) -> Int`,
    code: `fn ${name}(xs: List[Int]) -> Int = xs.fold(1, (acc, x) => acc * x)`,
    cases: [[], [5], [2, 3, 4], [-1, 6, 2], [0, 9, 9]].map((l) => [`${name}(${lit(l)})`, lit(l.reduce((p, x) => p * x, 1))]),
  };
});

fam("list.extreme-or", (r) => {
  const mx = r.chance(0.5);
  const name = mx ? r.pick(["maxOr", "biggestOr", "highestOr"]) : r.pick(["minOr", "smallestOr", "lowestOr"]);
  return {
    name,
    desc: `Return the ${mx ? "largest" : "smallest"} element, or the fallback when the list is empty.`,
    sig: `fn ${name}(xs: List[Int], fallback: Int) -> Int`,
    code: `fn ${name}(xs: List[Int], fallback: Int) -> Int = xs.${mx ? "max" : "min"}().unwrapOr(fallback)`,
    cases: [[[], -1], [[4], 0], [[3, 9, 2], 0], [[-5, -2, -9], 7]].map(([l, f]) => [`${name}(${lit(l)}, ${f})`, lit(l.length ? (mx ? Math.max(...l) : Math.min(...l)) : f)]),
  };
});

fam("list.first-or", (r) => {
  const name = r.pick(["firstOr", "headOr", "startOr"]);
  return {
    name,
    desc: "Return the first element, or the fallback when the list is empty.",
    sig: `fn ${name}(xs: List[Int], fallback: Int) -> Int`,
    code: `fn ${name}(xs: List[Int], fallback: Int) -> Int = xs.first().unwrapOr(fallback)`,
    cases: [[[], 8], [[4], 0], [[3, 9, 2], 0]].map(([l, f]) => [`${name}(${lit(l)}, ${f})`, lit(l.length ? l[0] : f)]),
  };
});

fam("list.take-drop", (r) => {
  const take = r.chance(0.5);
  const name = take ? r.pick(["firstN", "takeN", "headN"]) : r.pick(["skipN", "withoutFirstN", "dropN"]);
  const f = (l, n) => (take ? l.slice(0, Math.max(n, 0)) : l.slice(Math.max(n, 0)));
  return {
    name,
    desc: take ? "Return the first n elements (all of them when the list is shorter). n is not negative." : "Return the list without its first n elements (empty when the list is shorter). n is not negative.",
    sig: `fn ${name}(xs: List[Int], n: Int) -> List[Int]\n  requires n >= 0`,
    code: `fn ${name}(xs: List[Int], n: Int) -> List[Int]\n  requires n >= 0\n  = xs.${take ? "take" : "drop"}(n)`,
    cases: [[[1, 2, 3, 4], 2], [[1, 2], 5], [[], 3], [[7, 8, 9], 0]].map(([l, n]) => [`${name}(${lit(l)}, ${n})`, lit(f(l, n))]),
  };
});

fam("list.quantifiers", (r) => {
  const k = r.int(0, 12);
  const kind = r.pick(["allBelow", "anyAbove", "noneEqual", "allPositive", "anyNegative"]);
  const spec = {
    allBelow: [`allBelow${k}`, `Return true when every element is less than ${k}.`, `xs.all(x => x < ${k})`, (l) => l.every((x) => x < k)],
    anyAbove: [`anyAbove${k}`, `Return true when at least one element is greater than ${k}.`, `xs.any(x => x > ${k})`, (l) => l.some((x) => x > k)],
    noneEqual: [`lacks${k}`, `Return true when no element equals ${k}.`, `!xs.contains(${k})`, (l) => !l.includes(k)],
    allPositive: ["allPositive", "Return true when every element is greater than zero.", "xs.all(x => x > 0)", (l) => l.every((x) => x > 0)],
    anyNegative: ["hasNegative", "Return true when at least one element is below zero.", "xs.any(x => x < 0)", (l) => l.some((x) => x < 0)],
  }[kind];
  return {
    name: spec[0],
    desc: spec[1],
    sig: `fn ${spec[0]}(xs: List[Int]) -> Bool`,
    code: `fn ${spec[0]}(xs: List[Int]) -> Bool = ${spec[2]}`,
    cases: [[], [k], [k - 1, k + 1], [1, 2, 3], [-1, 5], r.list(4, -3, 14)].map((l) => [`${spec[0]}(${lit(l)})`, lit(spec[3](l))]),
  };
});

fam("list.index-or", (r) => {
  const name = r.pick(["positionOr", "indexOrDefault", "whereIs"]);
  return {
    name,
    desc: "Return the position (starting at 0) of the first element equal to target, or the fallback when it is missing.",
    sig: `fn ${name}(xs: List[Int], target: Int, fallback: Int) -> Int`,
    code: `fn ${name}(xs: List[Int], target: Int, fallback: Int) -> Int = xs.indexOf(target).unwrapOr(fallback)`,
    cases: [[[5, 6, 7], 6, -1], [[5, 6, 7], 9, -1], [[], 1, 0], [[2, 2, 2], 2, 9]].map(([l, t, f]) => [`${name}(${lit(l)}, ${t}, ${f})`, lit(l.includes(t) ? l.indexOf(t) : f)]),
  };
});

fam("list.sorted", (r) => {
  const desc = r.chance(0.5);
  const name = desc ? r.pick(["sortedDescending", "highestFirst"]) : r.pick(["sortedAscending", "lowestFirst", "ordered"]);
  return {
    name,
    desc: desc ? "Return the elements sorted from largest to smallest." : "Return the elements sorted from smallest to largest.",
    sig: `fn ${name}(xs: List[Int]) -> List[Int]`,
    code: desc ? `fn ${name}(xs: List[Int]) -> List[Int] = xs.sort().reverse()` : `fn ${name}(xs: List[Int]) -> List[Int] = xs.sort()`,
    cases: listCases(r, 5).map((l) => [`${name}(${lit(l)})`, lit([...l].sort((a, b) => (desc ? b - a : a - b)))]),
  };
});

fam("list.append-concat", (r) => {
  const kind = r.pick(["append", "concat", "twice", "reversed"]);
  const spec = {
    append: ["withExtra", "Return the list with the extra element added at the end.", "fn withExtra(xs: List[Int], extra: Int) -> List[Int] = xs.append(extra)", [[[1, 2], 3], [[], 1]], ([l, e]) => [...l, e], "xs: List[Int], extra: Int"],
    concat: ["joinLists", "Return all elements of a followed by all elements of b.", "fn joinLists(a: List[Int], b: List[Int]) -> List[Int] = a.concat(b)", [[[1], [2, 3]], [[], [4]], [[5, 6], []]], ([a, b]) => [...a, ...b], "a: List[Int], b: List[Int]"],
    twice: ["repeatedTwice", "Return the list followed by a copy of itself.", "fn repeatedTwice(xs: List[Int]) -> List[Int] = xs.concat(xs)", [[[1, 2]], [[]], [[7]]], ([l]) => [...l, ...l], "xs: List[Int]"],
    reversed: ["backwards", "Return the elements in reverse order.", "fn backwards(xs: List[Int]) -> List[Int] = xs.reverse()", [[[1, 2, 3]], [[]], [[4, 4, 5]]], ([l]) => [...l].reverse(), "xs: List[Int]"],
  }[kind];
  return {
    name: spec[0],
    desc: spec[1],
    sig: `fn ${spec[0]}(${spec[5]}) -> List[Int]`,
    code: spec[2],
    cases: spec[3].map((args) => [`${spec[0]}(${args.map(lit).join(", ")})`, lit(spec[4](args))]),
  };
});

fam("list.pair-up", (r) => {
  const name = r.pick(["pairUp", "zipTogether", "matchUp"]);
  return {
    name,
    desc: "Pair the elements of a and b by position, as tuples (stopping at the shorter list).",
    sig: `fn ${name}(a: List[Int], b: List[Int]) -> List[(Int, Int)]`,
    code: `fn ${name}(a: List[Int], b: List[Int]) -> List[(Int, Int)] = a.zip(b)`,
    cases: [[[1, 2, 3], [4, 5, 6]], [[1, 2], [9]], [[], [1]]].map(([a, b]) => [`${name}(${lit(a)}, ${lit(b)})`, lit(a.slice(0, Math.min(a.length, b.length)).map((x, i) => tup(x, b[i])))]),
  };
});

fam("list.weighted-sum", (r) => {
  const name = r.pick(["weightedSum", "positionWeighted", "indexTimesValue"]);
  return {
    name,
    desc: "Return the sum of index * value over the list, where the first element has index 0.",
    sig: `fn ${name}(xs: List[Int]) -> Int`,
    code: `fn ${name}(xs: List[Int]) -> Int = xs.enumerate().map(p => p.0 * p.1).sum()`,
    cases: [[], [5], [1, 2, 3], [4, 0, 2, 7]].map((l) => [`${name}(${lit(l)})`, lit(l.reduce((s, x, i) => s + i * x, 0))]),
  };
});

fam("list.range-sum", (r) => {
  const name = r.pick(["sumFromTo", "rangeTotal", "sumBetween"]);
  return {
    name,
    desc: "Return the sum of the integers from lo up to but not including hi (0 when hi is not above lo).",
    sig: `fn ${name}(lo: Int, hi: Int) -> Int`,
    code: `fn ${name}(lo: Int, hi: Int) -> Int = List.range(lo, hi).sum()`,
    cases: [[1, 5], [0, 0], [3, 4], [5, 2], [-2, 3]].map(([lo, hi]) => [`${name}(${lo}, ${hi})`, lit(hi > lo ? Array.from({ length: hi - lo }, (_, i) => lo + i).reduce((s, x) => s + x, 0) : 0)]),
  };
});

fam("list.recursion", (r) => {
  const kind = r.pick(["sum", "length", "countBelow", "containsK", "maxOfNonEmpty"]);
  const k = r.int(2, 20);
  const spec = {
    sum: ["totalOf", "Return the sum of the elements, writing the recursion yourself with a match on the list.", "fn totalOf(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [h, ..t] => h + totalOf(t)\n}", (l) => l.reduce((s, x) => s + x, 0), "Int"],
    length: ["sizeOf", "Return how many elements the list has, writing the recursion yourself with a match on the list.", "fn sizeOf(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [_, ..t] => 1 + sizeOf(t)\n}", (l) => l.length, "Int"],
    countBelow: [`howManyBelow${k}`, `Return how many elements are below ${k}, writing the recursion yourself with a match on the list.`, `fn howManyBelow${k}(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [h, ..t] => (if h < ${k} { 1 } else { 0 }) + howManyBelow${k}(t)\n}`, (l) => l.filter((x) => x < k).length, "Int"],
    containsK: [`hasValue${k}`, `Return true when ${k} occurs in the list, writing the recursion yourself with a match on the list.`, `fn hasValue${k}(xs: List[Int]) -> Bool = match xs {\n  [] => false\n  [h, ..t] => h == ${k} || hasValue${k}(t)\n}`, (l) => l.includes(k), "Bool"],
    maxOfNonEmpty: ["biggest", "Return the largest element of a list that is not empty, writing the recursion yourself.", "fn biggest(xs: List[Int]) -> Int\n  requires xs.len() > 0\n  = match xs {\n    [h] => h\n    [h, ..t] => h.max(biggest(t))\n    [] => 0\n  }", (l) => Math.max(...l), "Int"],
  }[kind];
  const nonEmpty = kind === "maxOfNonEmpty";
  const ls = [[1], [3, 9, 2], r.list(4, 0, 25), [k, 1, k]];
  if (!nonEmpty) ls.unshift([]);
  return {
    name: spec[0],
    desc: spec[1],
    sig: `fn ${spec[0]}(xs: List[Int]) -> ${spec[4]}${nonEmpty ? "\n  requires xs.len() > 0" : ""}`,
    code: spec[2],
    cases: ls.map((l) => [`${spec[0]}(${lit(l)})`, lit(spec[3](l))]),
  };
});

fam("list.stack", (r) => {
  const kind = r.pick(["push", "pop", "peek"]);
  const spec = {
    push: ["push", "Push an item on a stack kept as a list whose last element is the top: return the list with the item added at the end.", "fn push(stack: List[Int], item: Int) -> List[Int] = stack.append(item)", [[[1, 2], 3], [[], 9]], ([s, i]) => [...s, i], "stack: List[Int], item: Int", "List[Int]", ""],
    pop: ["pop", "Pop a stack kept as a list whose last element is the top: return the list without its last element. The stack is not empty.", "fn pop(stack: List[Int]) -> List[Int]\n  requires stack.len() > 0\n  = stack.take(stack.len() - 1)", [[[1, 2, 3]], [[7]]], ([s]) => s.slice(0, -1), "stack: List[Int]", "List[Int]", "\n  requires stack.len() > 0"],
    peek: ["top", "Return the top of a stack kept as a list whose last element is the top. The stack is not empty.", "fn top(stack: List[Int]) -> Int\n  requires stack.len() > 0\n  = stack.get(stack.len() - 1)", [[[1, 2, 3]], [[7]]], ([s]) => s[s.length - 1], "stack: List[Int]", "Int", "\n  requires stack.len() > 0"],
  }[kind];
  return {
    name: spec[0],
    desc: spec[1],
    sig: `fn ${spec[0]}(${spec[5]}) -> ${spec[6]}${spec[7]}`,
    code: spec[2],
    cases: spec[3].map((args) => [`${spec[0]}(${args.map(lit).join(", ")})`, lit(spec[4](args))]),
  };
});

// ---------------------------------------------------------------------------------------------- text
fam("str.shout", (r) => {
  const suffix = r.pick(["!", "?", "!!", "."]);
  const name = r.pick(["shout", "loud", "emphasize"]);
  return {
    name,
    desc: `Return the text in capital letters followed by "${suffix}".`,
    sig: `fn ${name}(s: Str) -> Str`,
    code: `fn ${name}(s: Str) -> Str = s.upper() ++ "${suffix}"`,
    cases: ["hello", "Tez", "", "a b"].map((s) => [`${name}(${lit(s)})`, lit(s.toUpperCase() + suffix)]),
  };
});

fam("str.greet", (r) => {
  const hello = r.pick(["Hello", "Welcome", "Good day", "Hi", "Greetings"]);
  const name = r.pick(["greet", "greeting", "welcomeText"]);
  return {
    name,
    desc: `Return "${hello}, " followed by the name and an exclamation mark.`,
    sig: `fn ${name}(who: Str) -> Str`,
    code: `fn ${name}(who: Str) -> Str = "${hello}, " ++ who ++ "!"`,
    cases: ["Ada", "world", ""].map((s) => [`${name}(${lit(s)})`, lit(`${hello}, ${s}!`)]),
  };
});

fam("str.first-char", (r) => {
  const last = r.chance(0.4);
  const name = last ? r.pick(["lastChar", "finalLetter"]) : r.pick(["firstChar", "initial", "firstLetter"]);
  return {
    name,
    desc: `Return the ${last ? "last" : "first"} character of the text as a one-character text. The text is not empty.`,
    sig: `fn ${name}(s: Str) -> Str\n  requires s.len() > 0`,
    code: last ? `fn ${name}(s: Str) -> Str\n  requires s.len() > 0\n  = s.slice(s.len() - 1, s.len())` : `fn ${name}(s: Str) -> Str\n  requires s.len() > 0\n  = s.slice(0, 1)`,
    cases: ["tez", "a", "Hello"].map((s) => [`${name}(${lit(s)})`, lit(last ? s[s.length - 1] : s[0])]),
  };
});

fam("str.repeat-with", (r) => {
  const name = r.pick(["repeatText", "stamp", "echoTimes"]);
  return {
    name,
    desc: "Return the text repeated n times, one after the other. n is not negative.",
    sig: `fn ${name}(s: Str, n: Int) -> Str\n  requires n >= 0`,
    code: `fn ${name}(s: Str, n: Int) -> Str\n  requires n >= 0\n  = s.repeat(n)`,
    cases: [["ab", 3], ["x", 0], ["", 5], ["go ", 2]].map(([s, n]) => [`${name}(${lit(s)}, ${n})`, lit(s.repeat(n))]),
  };
});

fam("str.checks", (r) => {
  const w = r.pick(["tez", "app", "data", "go", "web"]);
  const kind = r.pick(["blank", "prefix", "suffix", "contains", "tooLong", "emptyOrShort"]);
  const k = r.int(3, 12);
  const spec = {
    blank: ["isBlank", "Return true when the text is empty or only spaces.", "s.trim().isEmpty()", (s) => s.trim() === "", ["", "  ", "a", " b "]],
    prefix: [`startsWith${w[0].toUpperCase()}${w.slice(1)}`, `Return true when the text starts with "${w}".`, `s.startsWith("${w}")`, (s) => s.startsWith(w), [w, `${w}x`, `x${w}`, ""]],
    suffix: [`endsWith${w[0].toUpperCase()}${w.slice(1)}`, `Return true when the text ends with "${w}".`, `s.endsWith("${w}")`, (s) => s.endsWith(w), [w, `${w}x`, `x${w}`, ""]],
    contains: [`mentions${w[0].toUpperCase()}${w.slice(1)}`, `Return true when the text contains "${w}" anywhere.`, `s.contains("${w}")`, (s) => s.includes(w), [`a ${w} b`, w, "none", ""]],
    tooLong: [`tooLongFor${k}`, `Return true when the text has more than ${k} characters.`, `s.len() > ${k}`, (s) => s.length > k, ["", "x".repeat(k), "x".repeat(k + 1)]],
    emptyOrShort: [`isShort${k}`, `Return true when the text has at most ${k} characters.`, `s.len() <= ${k}`, (s) => s.length <= k, ["", "x".repeat(k), "x".repeat(k + 2)]],
  }[kind];
  return {
    name: spec[0],
    desc: spec[1],
    sig: `fn ${spec[0]}(s: Str) -> Bool`,
    code: `fn ${spec[0]}(s: Str) -> Bool = ${spec[2]}`,
    cases: spec[4].map((s) => [`${spec[0]}(${lit(s)})`, lit(spec[3](s))]),
  };
});

fam("str.wrap", (r) => {
  const [l, rr] = r.pick([["(", ")"], ["[", "]"], ["<", ">"], ["\"", "\""], ["*", "*"]]);
  const name = r.pick(["wrapText", "surround", "bracketed"]);
  const lq = l === "\"" ? "\\\"" : l;
  const rq = rr === "\"" ? "\\\"" : rr;
  return {
    name,
    desc: `Return the text with "${l}" before it and "${rr}" after it.`,
    sig: `fn ${name}(s: Str) -> Str`,
    code: `fn ${name}(s: Str) -> Str = "${lq}" ++ s ++ "${rq}"`,
    cases: ["x", "", "two words"].map((s) => [`${name}(${lit(s)})`, lit(l + s + rr)]),
  };
});

fam("str.shorten", (r) => {
  const name = r.pick(["shorten", "truncate", "preview"]);
  return {
    name,
    desc: "Return the text unchanged when it has at most limit characters; otherwise return its first limit characters followed by \"...\". The limit is not negative.",
    sig: `fn ${name}(s: Str, limit: Int) -> Str\n  requires limit >= 0`,
    code: `fn ${name}(s: Str, limit: Int) -> Str\n  requires limit >= 0\n  = if s.len() <= limit { s } else { s.slice(0, limit) ++ "..." }`,
    cases: [["hello", 10], ["hello world", 5], ["abc", 3], ["abc", 0], ["", 2]].map(([s, n]) => [`${name}(${lit(s)}, ${n})`, lit(s.length <= n ? s : s.slice(0, n) + "...")]),
  };
});

fam("str.parse", (r) => {
  const name = r.pick(["parseOr", "toNumberOr", "readInt"]);
  return {
    name,
    desc: "Parse the text as a whole number, or return the fallback when it is not one.",
    sig: `fn ${name}(s: Str, fallback: Int) -> Int`,
    code: `fn ${name}(s: Str, fallback: Int) -> Int = s.toInt().unwrapOr(fallback)`,
    cases: [["42", 0], ["x", -1], ["", 7], ["-5", 0]].map(([s, f]) => [`${name}(${lit(s)}, ${f})`, lit(/^-?\d+$/.test(s) ? Number(s) : f)]),
  };
});

fam("str.yes-no", (r) => {
  const [t, f] = r.pick([["yes", "no"], ["on", "off"], ["open", "closed"], ["enabled", "disabled"], ["true", "false"]]);
  const name = r.pick([`${t}Or${f[0].toUpperCase()}${f.slice(1)}`, "label", "describe"]);
  return {
    name,
    desc: `Return "${t}" when the flag is true and "${f}" when it is false.`,
    sig: `fn ${name}(flag: Bool) -> Str`,
    code: `fn ${name}(flag: Bool) -> Str = if flag { "${t}" } else { "${f}" }`,
    cases: [true, false].map((b) => [`${name}(${b})`, lit(b ? t : f)]),
  };
});

fam("str.words", (r) => {
  const kind = r.pick(["longerThan", "totalLength", "bullets", "numbers", "commaNumbers", "firstWord"]);
  const k = r.int(2, 5);
  const ws = [["a", "tez", "language"], ["go"], [], ["one", "two", "three", "four"]];
  const spec = {
    longerThan: [`countLongerThan${k}`, `Return how many of the words have more than ${k} characters.`, `fn countLongerThan${k}(words: List[Str]) -> Int = words.count(w => w.len() > ${k})`, ws, (l) => l.filter((w) => w.length > k).length, "words: List[Str]", "Int"],
    totalLength: ["totalLength", "Return the total number of characters in all the words.", "fn totalLength(words: List[Str]) -> Int = words.map(w => w.len()).sum()", ws, (l) => l.reduce((s, w) => s + w.length, 0), "words: List[Str]", "Int"],
    bullets: ["bulletList", "Return the words as lines starting with \"- \", joined by newlines.", "fn bulletList(words: List[Str]) -> Str = words.map(w => \"- \" ++ w).join(\"\\n\")", ws, (l) => l.map((w) => `- ${w}`).join("\n"), "words: List[Str]", "Str"],
    numbers: ["numbersAsText", "Return every number written as text.", "fn numbersAsText(xs: List[Int]) -> List[Str] = xs.map(x => x.toStr())", [[1, 20], [], [-3, 0, 7]], (l) => l.map(String), "xs: List[Int]", "List[Str]"],
    commaNumbers: ["commaSeparated", "Return the numbers written out and joined with \", \".", "fn commaSeparated(xs: List[Int]) -> Str = xs.map(x => x.toStr()).join(\", \")", [[1, 20], [], [-3, 0, 7], [5]], (l) => l.join(", "), "xs: List[Int]", "Str"],
    firstWord: ["firstWord", "Return the first word of a sentence whose words are separated by single spaces (the whole text when there is only one word).", "fn firstWord(sentence: Str) -> Str = sentence.split(\" \").first().unwrapOr(\"\")", [["hello brave world"], ["solo"], [""]], (a) => a[0].split(" ")[0], "sentence: Str", "Str"],
  }[kind];
  return {
    name: spec[0],
    desc: spec[1],
    sig: `fn ${spec[0]}(${spec[5]}) -> ${spec[6]}`,
    code: spec[2],
    cases: spec[3].map((a) => [`${spec[0]}(${lit(Array.isArray(a) && kind !== "firstWord" ? a : kind === "firstWord" ? a[0] : a)})`, lit(spec[4](a))]),
  };
});

// ---------------------------------------------------------------------------------------------- option / result
fam("opt.safe-divide", (r) => {
  const name = r.pick(["safeDivide", "divideOrNone", "tryDivide"]);
  return {
    name,
    desc: "Divide a by b; return None when b is zero and Some(quotient) otherwise.",
    sig: `fn ${name}(a: Int, b: Int) -> Option[Int]`,
    code: `fn ${name}(a: Int, b: Int) -> Option[Int] = if b == 0 { None } else { Some(a / b) }`,
    cases: [[7, 2], [7, 0], [-9, 3], [0, 5]].flatMap(([a, b]) => [[`${name}(${a}, ${b}).unwrapOr(-99)`, lit(b === 0 ? -99 : idiv(a, b))], [`${name}(${a}, ${b}).isSome()`, lit(b !== 0)]]),
  };
});

fam("opt.find-first", (r) => {
  const k = r.int(2, 15);
  const name = r.pick([`firstAbove${k}`, `findAbove${k}`]);
  return {
    name,
    desc: `Return the first element greater than ${k}, or None when there is none.`,
    sig: `fn ${name}(xs: List[Int]) -> Option[Int]`,
    code: `fn ${name}(xs: List[Int]) -> Option[Int] = xs.filter(x => x > ${k}).first()`,
    cases: [[], [1, k + 1], [k + 5, k + 9], [0, -3]].flatMap((l) => [[`${name}(${lit(l)}).unwrapOr(-1)`, lit(l.find((x) => x > k) ?? -1)], [`${name}(${lit(l)}).isNone()`, lit(!l.some((x) => x > k))]]),
  };
});

fam("opt.parse-result", (r) => {
  const name = r.pick(["parsePositive", "readCount", "parseQuantity"]);
  return {
    name,
    desc: "Parse the text as a whole number. Return Ok(n) when it is a number greater than zero, Err(\"not positive\") when it is a number that is not, and Err(\"not a number\") when it is not a number.",
    sig: `fn ${name}(s: Str) -> Result[Int, Str]`,
    code: `fn ${name}(s: Str) -> Result[Int, Str] = match s.toInt() {\n  Some(n) => if n > 0 { Ok(n) } else { Err("not positive") }\n  None => Err("not a number")\n}`,
    cases: ["12", "0", "-4", "abc", ""].flatMap((s) => {
      const n = /^-?\d+$/.test(s) ? Number(s) : null;
      return [[`${name}(${lit(s)}).isOk()`, lit(n !== null && n > 0)], [`${name}(${lit(s)}).unwrapOr(-1)`, lit(n !== null && n > 0 ? n : -1)]];
    }),
  };
});

fam("opt.describe", (r) => {
  const name = r.pick(["describe", "showOption", "label"]);
  const none = r.pick(["nothing", "empty", "missing"]);
  return {
    name,
    desc: `Return "${none}" for None and "got " followed by the number for Some(number).`,
    sig: `fn ${name}(o: Option[Int]) -> Str`,
    code: `fn ${name}(o: Option[Int]) -> Str = match o {\n  Some(n) => "got " ++ n.toStr()\n  None => "${none}"\n}`,
    cases: [[raw("Some(5)"), "got 5"], [raw("None"), none], [raw("Some(-2)"), "got -2"]].map(([a, want]) => [`${name}(${lit(a)})`, lit(want)]),
  };
});

// ---------------------------------------------------------------------------------------------- records and variants
const ENUMS = [
  { type: "Light", vs: ["Red", "Green", "Yellow"], fn: "next", ret: "Light", map: { Red: "Green", Green: "Yellow", Yellow: "Red" }, desc: "the light that comes after this one: Red, then Green, then Yellow, then Red again" },
  { type: "Direction", vs: ["North", "East", "South", "West"], fn: "turnRight", ret: "Direction", map: { North: "East", East: "South", South: "West", West: "North" }, desc: "the direction after turning right (clockwise)" },
  { type: "Direction", vs: ["North", "East", "South", "West"], fn: "opposite", ret: "Direction", map: { North: "South", South: "North", East: "West", West: "East" }, desc: "the opposite direction" },
  { type: "Weekday", vs: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"], fn: "isWeekend", ret: "Bool", map: { Mon: false, Tue: false, Wed: false, Thu: false, Fri: false, Sat: true, Sun: true }, desc: "true for Sat and Sun" },
  { type: "Size", vs: ["Small", "Medium", "Large"], fn: "price", ret: "Int", map: { Small: 3, Medium: 4, Large: 6 }, desc: "the price: 3 for Small, 4 for Medium, 6 for Large" },
  { type: "Coin", vs: ["Penny", "Nickel", "Dime", "Quarter"], fn: "cents", ret: "Int", map: { Penny: 1, Nickel: 5, Dime: 10, Quarter: 25 }, desc: "its value in cents: 1, 5, 10 and 25" },
  { type: "Suit", vs: ["Hearts", "Spades", "Clubs", "Diamonds"], fn: "isRed", ret: "Bool", map: { Hearts: true, Diamonds: true, Spades: false, Clubs: false }, desc: "true for Hearts and Diamonds" },
  { type: "Level", vs: ["Low", "Mid", "High"], fn: "weight", ret: "Int", map: { Low: 1, Mid: 5, High: 10 }, desc: "the weight: 1 for Low, 5 for Mid, 10 for High" },
  { type: "Mode", vs: ["Light", "Dark"], fn: "toggle", ret: "Mode", map: { Light: "Dark", Dark: "Light" }, desc: "the other mode" },
  { type: "Status", vs: ["Draft", "Review", "Published"], fn: "label", ret: "Str", map: { Draft: "In draft", Review: "Waiting for review", Published: "Live" }, desc: "a label: \"In draft\", \"Waiting for review\" or \"Live\"" },
];
fam("adt.enum-function", (r) => {
  const e = r.pick(ENUMS);
  const rv = (v) => (typeof v === "string" && e.ret === "Str" ? lit(v) : typeof v === "string" ? v : lit(v));
  const arms = e.vs.map((v) => `  ${v} => ${rv(e.map[v])}`).join("\n");
  return {
    name: e.fn,
    types: `type ${e.type} = ${e.vs.join(" | ")}`,
    desc: `Given a value of type ${e.type}, return ${e.desc}.`,
    sig: `fn ${e.fn}(v: ${e.type}) -> ${e.ret}`,
    code: `type ${e.type} = ${e.vs.join(" | ")}\n\nfn ${e.fn}(v: ${e.type}) -> ${e.ret} = match v {\n${arms}\n}`,
    cases: e.vs.map((v) => [`${e.fn}(${v})`, rv(e.map[v])]),
  };
});

fam("adt.shapes", (r) => {
  const pick = r.shuffle(["Circle", "Square", "Rect", "Triangle"]).slice(0, r.int(2, 4));
  const arms = {
    Circle: ["Circle(Int)", "Circle(r) => 3 * r * r", (r_) => 3 * r_ * r_, [[2], [5]], "a circle with radius r has area 3 * r * r"],
    Square: ["Square(Int)", "Square(side) => side * side", (s) => s * s, [[3], [7]], "a square with side s has area s * s"],
    Rect: ["Rect(Int, Int)", "Rect(w, h) => w * h", (w, h) => w * h, [[3, 4], [10, 2]], "a rectangle with width w and height h has area w * h"],
    Triangle: ["Triangle(Int, Int)", "Triangle(b, h) => b * h / 2", (b, h) => idiv(b * h, 2), [[6, 3], [5, 5]], "a triangle with base b and height h has area b * h / 2 (rounded down)"],
  };
  return {
    name: "area",
    types: `type Shape = ${pick.map((p) => arms[p][0]).join(" | ")}`,
    desc: `Compute the area of a shape: ${pick.map((p) => arms[p][4]).join("; ")}.`,
    sig: "fn area(shape: Shape) -> Int",
    code: `type Shape = ${pick.map((p) => arms[p][0]).join(" | ")}\n\nfn area(shape: Shape) -> Int = match shape {\n${pick.map((p) => `  ${arms[p][1]}`).join("\n")}\n}`,
    cases: pick.flatMap((p) => arms[p][3].map((a) => [`area(${p}(${a.join(", ")}))`, lit(arms[p][2](...a))])),
  };
});

fam("rec.point", (r) => {
  const kind = r.pick(["manhattan", "move", "swapXY", "isOrigin", "quadrant"]);
  const T = "type Point = { x: Int, y: Int }";
  const spec = {
    manhattan: ["manhattan", "Return the Manhattan distance between two points: |ax - bx| + |ay - by|.", "fn manhattan(a: Point, b: Point) -> Int\n  ensures result >= 0\n  = (a.x - b.x).abs() + (a.y - b.y).abs()", "a: Point, b: Point", "Int", "\n  ensures result >= 0", [[[1, 2], [4, 6]], [[0, 0], [0, 0]], [[-1, -1], [1, 1]]], (a, b) => Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]), (a, b) => `manhattan(Point { x: ${a[0]}, y: ${a[1]} }, Point { x: ${b[0]}, y: ${b[1]} })`],
    move: ["moveBy", "Return the point moved by dx horizontally and dy vertically.", "fn moveBy(p: Point, dx: Int, dy: Int) -> Point = Point { x: p.x + dx, y: p.y + dy }", "p: Point, dx: Int, dy: Int", "Point", "", [[[1, 2], 3, -1], [[0, 0], 0, 0]], null, null],
    swapXY: ["mirror", "Return the point with its x and y swapped.", "fn mirror(p: Point) -> Point = Point { x: p.y, y: p.x }", "p: Point", "Point", "", [[[1, 2]], [[5, -5]]], null, null],
    isOrigin: ["isOrigin", "Return true when the point is at (0, 0).", "fn isOrigin(p: Point) -> Bool = p.x == 0 && p.y == 0", "p: Point", "Bool", "", [[[0, 0]], [[0, 1]], [[3, 0]]], ([x, y]) => x === 0 && y === 0, ([x, y]) => `isOrigin(Point { x: ${x}, y: ${y} })`],
    quadrant: ["quadrant", "Return 1 when both coordinates are positive, 2 when only y is positive, 3 when neither is positive and 4 when only x is positive.", "fn quadrant(p: Point) -> Int = if p.x > 0 { if p.y > 0 { 1 } else { 4 } } else { if p.y > 0 { 2 } else { 3 } }", "p: Point", "Int", "", [[[1, 1]], [[-1, 2]], [[-3, -3]], [[2, -2]], [[0, 0]]], ([x, y]) => (x > 0 ? (y > 0 ? 1 : 4) : y > 0 ? 2 : 3), ([x, y]) => `quadrant(Point { x: ${x}, y: ${y} })`],
  }[kind];
  const P = (a) => `Point { x: ${a[0]}, y: ${a[1]} }`;
  let cases;
  if (kind === "move") cases = spec[6].flatMap(([p, dx, dy]) => [[`moveBy(${P(p)}, ${dx}, ${dy}).x`, lit(p[0] + dx)], [`moveBy(${P(p)}, ${dx}, ${dy}).y`, lit(p[1] + dy)]]);
  else if (kind === "swapXY") cases = spec[6].flatMap(([p]) => [[`mirror(${P(p)}).x`, lit(p[1])], [`mirror(${P(p)}).y`, lit(p[0])]]);
  else if (kind === "manhattan") cases = spec[6].map(([a, b]) => [spec[8](a, b), lit(spec[7](a, b))]);
  else cases = spec[6].map(([p]) => [spec[8](p), lit(spec[7](p))]);
  return { name: spec[0], types: T, desc: spec[1], sig: `fn ${spec[0]}(${spec[3]}) -> ${spec[4]}${spec[5]}`, code: `${T}\n\n${spec[2]}`, cases };
});

fam("rec.rect", (r) => {
  const kind = r.pick(["area", "perimeter", "isSquare", "scale"]);
  const T = "type Rect = { w: Int where it > 0, h: Int where it > 0 }";
  const R = ([w, h]) => `Rect { w: ${w}, h: ${h} }`;
  const dims = [[3, 4], [5, 5], [1, 9]];
  const spec = {
    area: ["rectArea", "Return the area of the rectangle.", "fn rectArea(r: Rect) -> Int = r.w * r.h", "Int", (d) => d[0] * d[1], (d) => `rectArea(${R(d)})`],
    perimeter: ["rectPerimeter", "Return the perimeter of the rectangle.", "fn rectPerimeter(r: Rect) -> Int = 2 * (r.w + r.h)", "Int", (d) => 2 * (d[0] + d[1]), (d) => `rectPerimeter(${R(d)})`],
    isSquare: ["isSquare", "Return true when the rectangle's sides are equal.", "fn isSquare(r: Rect) -> Bool = r.w == r.h", "Bool", (d) => d[0] === d[1], (d) => `isSquare(${R(d)})`],
    scale: ["doubled", "Return the rectangle with both sides doubled, then return its width.", "fn doubled(r: Rect) -> Int = (Rect { w: r.w * 2, h: r.h * 2 }).w", "Int", (d) => d[0] * 2, (d) => `doubled(${R(d)})`],
  }[kind];
  return { name: spec[0], types: T, desc: spec[1], sig: `fn ${spec[0]}(r: Rect) -> ${spec[3]}`, code: `${T}\n\n${spec[2]}`, cases: dims.map((d) => [spec[5](d), lit(spec[4](d))]) };
});

fam("rec.account", (r) => {
  const kind = r.pick(["deposit", "withdraw"]);
  const T = "type Account = { balance: Int where it >= 0 }";
  const A = (b) => `Account { balance: ${b} }`;
  const spec =
    kind === "deposit"
      ? { name: "deposit", desc: "Return the account with the amount added to its balance. The amount is not negative.", sig: "fn deposit(a: Account, amount: Int) -> Account\n  requires amount >= 0", code: `${T}\n\nfn deposit(a: Account, amount: Int) -> Account\n  requires amount >= 0\n  = Account { balance: a.balance + amount }`, cases: [[10, 5], [0, 0], [3, 100]].map(([b, x]) => [`deposit(${A(b)}, ${x}).balance`, lit(b + x)]) }
      : { name: "withdraw", desc: "Return the account with the amount taken from its balance. The amount is not negative and not more than the balance.", sig: "fn withdraw(a: Account, amount: Int) -> Account\n  requires amount >= 0 && amount <= a.balance", code: `${T}\n\nfn withdraw(a: Account, amount: Int) -> Account\n  requires amount >= 0 && amount <= a.balance\n  = Account { balance: a.balance - amount }`, cases: [[10, 5], [4, 4], [9, 0]].map(([b, x]) => [`withdraw(${A(b)}, ${x}).balance`, lit(b - x)]) };
  return { name: spec.name, types: T, desc: spec.desc, sig: spec.sig + (spec.sig.includes("->") ? "" : ""), code: spec.code, cases: spec.cases };
});

fam("adt.expression", (r) => {
  const kind = r.pick(["add-mul", "with-neg", "with-sub"]);
  const spec = {
    "add-mul": { T: "type Expr = Num(Int) | Add(Expr, Expr) | Mul(Expr, Expr)", arms: "  Num(n) => n\n  Add(a, b) => eval(a) + eval(b)\n  Mul(a, b) => eval(a) * eval(b)", d: "a number, the sum of two expressions, or the product of two expressions", cases: [["Num(4)", 4], ["Add(Num(2), Num(3))", 5], ["Mul(Add(Num(1), Num(2)), Num(5))", 15]] },
    "with-neg": { T: "type Expr = Num(Int) | Add(Expr, Expr) | Neg(Expr)", arms: "  Num(n) => n\n  Add(a, b) => eval(a) + eval(b)\n  Neg(a) => 0 - eval(a)", d: "a number, the sum of two expressions, or the negation of an expression", cases: [["Num(4)", 4], ["Neg(Num(3))", -3], ["Add(Num(2), Neg(Num(7)))", -5]] },
    "with-sub": { T: "type Expr = Num(Int) | Sub(Expr, Expr) | Mul(Expr, Expr)", arms: "  Num(n) => n\n  Sub(a, b) => eval(a) - eval(b)\n  Mul(a, b) => eval(a) * eval(b)", d: "a number, the difference of two expressions, or the product of two expressions", cases: [["Num(4)", 4], ["Sub(Num(9), Num(3))", 6], ["Mul(Sub(Num(5), Num(2)), Num(4))", 12]] },
  }[kind];
  return {
    name: "eval",
    types: spec.T,
    desc: `An expression is ${spec.d}. Evaluate it.`,
    sig: "fn eval(e: Expr) -> Int",
    code: `${spec.T}\n\nfn eval(e: Expr) -> Int = match e {\n${spec.arms}\n}`,
    cases: spec.cases.map(([e, v]) => [`eval(${e})`, lit(v)]),
  };
});

// ---------------------------------------------------------------------------------------------- refinement, higher-order, tuples
fam("refine.percent", (r) => {
  const name = r.pick(["toPercent", "asPercent", "limitPercent"]);
  return {
    name,
    types: "type Percent = Int where it >= 0 && it <= 100",
    desc: "A Percent is a whole number from 0 to 100. Turn any integer into a Percent by limiting it to that range.",
    sig: `fn ${name}(n: Int) -> Percent`,
    code: `type Percent = Int where it >= 0 && it <= 100\n\nfn ${name}(n: Int) -> Percent = if n < 0 { 0 } else if n > 100 { 100 } else { n }`,
    cases: [-5, 0, 42, 100, 250].map((v) => [`${name}(${v})`, lit(Math.min(100, Math.max(0, v)))]),
  };
});

fam("refine.positive", (r) => {
  const name = r.pick(["toPositive", "atLeastOne", "ensurePositive"]);
  return {
    name,
    types: "type Positive = Int where it > 0",
    desc: "A Positive is a whole number above zero. Turn any integer into a Positive by using 1 for every value below 1.",
    sig: `fn ${name}(n: Int) -> Positive`,
    code: `type Positive = Int where it > 0\n\nfn ${name}(n: Int) -> Positive = if n < 1 { 1 } else { n }`,
    cases: [-3, 0, 1, 8].map((v) => [`${name}(${v})`, lit(Math.max(1, v))]),
  };
});

fam("hof.apply", (r) => {
  const kind = r.pick(["twice", "compose", "applyTo", "mapPair"]);
  const spec = {
    twice: ["applyTwice", "Apply the function f to x, then apply f to that result.", "fn applyTwice(f: (Int) -> Int, x: Int) -> Int = f(f(x))", [["x => x + 3", 10, 16], ["x => x * 2", 5, 20], ["x => 0 - x", 7, 7]], (f, x) => `applyTwice(${f}, ${x})`],
    compose: ["composeThenAdd", "Apply f to x and add k to the result.", "fn composeThenAdd(f: (Int) -> Int, x: Int, k: Int) -> Int = f(x) + k", [["x => x * x", 4, 1, 17], ["x => x - 1", 1, 5, 5]], null],
    applyTo: ["applyToAll", "Apply the function f to every element.", "fn applyToAll(f: (Int) -> Int, xs: List[Int]) -> List[Int] = xs.map(f)", [["x => x + 1", [1, 2, 3], [2, 3, 4]], ["x => x * x", [], []], ["x => 0 - x", [4, -5], [-4, 5]]], (f, xs) => `applyToAll(${f}, ${lit(xs)})`],
    mapPair: ["onBoth", "Apply the function f to both numbers of the pair and return the new pair.", "fn onBoth(f: (Int) -> Int, p: (Int, Int)) -> (Int, Int) = (f(p.0), f(p.1))", [["x => x + 1", tup(1, 2), tup(2, 3)], ["x => x * 3", tup(0, -1), tup(0, -3)]], (f, p) => `onBoth(${f}, ${lit(p)})`],
  }[kind];
  const sigs = {
    twice: "f: (Int) -> Int, x: Int",
    compose: "f: (Int) -> Int, x: Int, k: Int",
    applyTo: "f: (Int) -> Int, xs: List[Int]",
    mapPair: "f: (Int) -> Int, p: (Int, Int)",
  };
  const rets = { twice: "Int", compose: "Int", applyTo: "List[Int]", mapPair: "(Int, Int)" };
  const cases = spec[3].map((c) => {
    if (kind === "compose") return [`composeThenAdd(${c[0]}, ${c[1]}, ${c[2]})`, lit(c[3])];
    return [spec[4](c[0], c[1]), lit(c[2])];
  });
  return { name: spec[0], desc: spec[1], sig: `fn ${spec[0]}(${sigs[kind]}) -> ${rets[kind]}`, code: spec[2], cases };
});

fam("tuple.divmod", (r) => {
  const kind = r.pick(["divmod", "minMax", "swap"]);
  const spec = {
    divmod: ["divMod", "Return the quotient and the remainder of a divided by b as a pair. b is positive.", "fn divMod(a: Int, b: Int) -> (Int, Int)\n  requires b > 0\n  = (a / b, a % b)", "a: Int, b: Int", "(Int, Int)", "\n  requires b > 0", [[17, 5], [10, 2], [3, 9]], ([a, b]) => tup(idiv(a, b), imod(a, b)), ([a, b]) => `divMod(${a}, ${b})`],
    minMax: ["lowAndHigh", "Return the smaller and the larger of two numbers as a pair (smaller first).", "fn lowAndHigh(a: Int, b: Int) -> (Int, Int) = (a.min(b), a.max(b))", "a: Int, b: Int", "(Int, Int)", "", [[5, 2], [1, 9], [4, 4]], ([a, b]) => tup(Math.min(a, b), Math.max(a, b)), ([a, b]) => `lowAndHigh(${a}, ${b})`],
    swap: ["swapPair", "Return the pair with its two numbers swapped.", "fn swapPair(p: (Int, Int)) -> (Int, Int) = (p.1, p.0)", "p: (Int, Int)", "(Int, Int)", "", [[tup(1, 2)], [tup(-3, 0)]], ([p]) => tup(p.tup[1], p.tup[0]), ([p]) => `swapPair(${lit(p)})`],
  }[kind];
  return { name: spec[0], desc: spec[1], sig: `fn ${spec[0]}(${spec[3]}) -> ${spec[4]}${spec[5]}`, code: spec[2], cases: spec[6].map((a) => [spec[8](a), lit(spec[7](a))]) };
});

// ---------------------------------------------------------------------------------------------- proof-heavy functions
// Functions whose contracts the prover needs (indexing, division, termination): the ones models get wrong most.
fam("proof.safe-index", (r) => {
  const kind = r.pick(["itemAt", "second", "third", "lastItem", "middle", "firstTwoSum"]);
  const spec = {
    itemAt: ["itemAt", "Return the element at position i (the first element is at position 0). i is a valid position.", "fn itemAt(xs: List[Int], i: Int) -> Int\n  requires 0 <= i && i < xs.len()\n  = xs.get(i)", "xs: List[Int], i: Int", "\n  requires 0 <= i && i < xs.len()", [[[5, 6, 7], 0], [[5, 6, 7], 2], [[9], 0]], ([l, i]) => l[i]],
    second: ["secondItem", "Return the second element of a list that has at least two elements.", "fn secondItem(xs: List[Int]) -> Int\n  requires xs.len() >= 2\n  = xs.get(1)", "xs: List[Int]", "\n  requires xs.len() >= 2", [[[1, 2]], [[7, 8, 9]]], ([l]) => l[1]],
    third: ["thirdItem", "Return the third element of a list that has at least three elements.", "fn thirdItem(xs: List[Int]) -> Int\n  requires xs.len() >= 3\n  = xs.get(2)", "xs: List[Int]", "\n  requires xs.len() >= 3", [[[1, 2, 3]], [[7, 8, 9, 10]]], ([l]) => l[2]],
    lastItem: ["lastItem", "Return the last element of a list that is not empty.", "fn lastItem(xs: List[Int]) -> Int\n  requires xs.len() > 0\n  = xs.get(xs.len() - 1)", "xs: List[Int]", "\n  requires xs.len() > 0", [[[4]], [[1, 2, 3]]], ([l]) => l[l.length - 1]],
    middle: ["middleItem", "Return the middle element of a list that is not empty (for an even length, the one just after the middle).", "fn middleItem(xs: List[Int]) -> Int\n  requires xs.len() > 0\n  = xs.get(xs.len() / 2)", "xs: List[Int]", "\n  requires xs.len() > 0", [[[4]], [[1, 2, 3]], [[1, 2, 3, 4]]], ([l]) => l[Math.trunc(l.length / 2)]],
    firstTwoSum: ["firstTwoSum", "Return the sum of the first two elements of a list that has at least two elements.", "fn firstTwoSum(xs: List[Int]) -> Int\n  requires xs.len() >= 2\n  = xs.get(0) + xs.get(1)", "xs: List[Int]", "\n  requires xs.len() >= 2", [[[1, 2]], [[7, 8, 9]]], ([l]) => l[0] + l[1]],
  }[kind];
  return { name: spec[0], desc: spec[1], sig: `fn ${spec[0]}(${spec[3]}) -> Int${spec[4]}`, code: spec[2], cases: spec[5].map((a) => [`${spec[0]}(${a.map(lit).join(", ")})`, lit(spec[6](a))]) };
});

fam("proof.safe-division", (r) => {
  const kind = r.pick(["ratio", "perItem", "wrap", "cycleNext", "shareEach", "stepsNeeded"]);
  const spec = {
    ratio: ["ratio", "Return a divided by b (rounded toward zero). b is not zero.", "fn ratio(a: Int, b: Int) -> Int\n  requires b != 0\n  = a / b", "a: Int, b: Int", "\n  requires b != 0", [[7, 2], [-9, 3], [0, 4]], ([a, b]) => idiv(a, b)],
    perItem: ["perItem", "Return the total divided by the number of items, rounded down. The number of items is positive.", "fn perItem(total: Int, count: Int) -> Int\n  requires count > 0\n  = total / count", "total: Int, count: Int", "\n  requires count > 0", [[10, 3], [9, 3], [0, 5]], ([a, b]) => idiv(a, b)],
    wrap: ["wrapAround", "Return n modulo size, so that the result is from 0 up to size - 1. n is not negative and size is positive.", "fn wrapAround(n: Int, size: Int) -> Int\n  requires n >= 0 && size > 0\n  ensures result >= 0 && result < size\n  = n % size", "n: Int, size: Int", "\n  requires n >= 0 && size > 0\n  ensures result >= 0 && result < size", [[7, 5], [10, 5], [0, 3]], ([a, b]) => a % b],
    cycleNext: ["cycleNext", "Return the position after i in a cycle of the given size (the last position is followed by 0). i is a valid position.", "fn cycleNext(i: Int, size: Int) -> Int\n  requires size > 0 && i >= 0 && i < size\n  ensures result >= 0 && result < size\n  = (i + 1) % size", "i: Int, size: Int", "\n  requires size > 0 && i >= 0 && i < size\n  ensures result >= 0 && result < size", [[0, 3], [2, 3], [1, 2]], ([i, n]) => (i + 1) % n],
    shareEach: ["shareEach", "Split an amount between people and return what each person gets, rounded down. There is at least one person.", "fn shareEach(amount: Int, people: Int) -> Int\n  requires people >= 1\n  = amount / people", "amount: Int, people: Int", "\n  requires people >= 1", [[100, 3], [5, 5], [2, 4]], ([a, b]) => idiv(a, b)],
    stepsNeeded: ["stepsNeeded", "Return how many steps of the given size are needed to cover a distance, rounding up. The distance is not negative and the step is positive.", "fn stepsNeeded(distance: Int, step: Int) -> Int\n  requires distance >= 0 && step > 0\n  = (distance + step - 1) / step", "distance: Int, step: Int", "\n  requires distance >= 0 && step > 0", [[10, 3], [9, 3], [0, 2]], ([d, s]) => Math.ceil(d / s)],
  }[kind];
  return { name: spec[0], desc: spec[1], sig: `fn ${spec[0]}(${spec[3]}) -> Int${spec[4]}`, code: spec[2], cases: spec[5].map((a) => [`${spec[0]}(${a.join(", ")})`, lit(spec[6](a))]) };
});

fam("proof.termination", (r) => {
  const kind = r.pick(["halvings", "pow2", "repeatDown", "sumHalves", "lengthRec", "countUp"]);
  const spec = {
    halvings: ["halvings", "Return how many times n can be halved (integer division by 2) before it reaches 1. n is at least 1.", "fn halvings(n: Int) -> Int\n  requires n >= 1\n  decreases n\n  = if n <= 1 { 0 } else { 1 + halvings(n / 2) }", "n: Int", "\n  requires n >= 1", [[1], [2], [8], [100]], ([n]) => Math.floor(Math.log2(n))],
    pow2: ["twoToThe", "Return 2 raised to the power n, computed by recursion. n is not negative.", "fn twoToThe(n: Int) -> Int\n  requires n >= 0\n  decreases n\n  = if n == 0 { 1 } else { 2 * twoToThe(n - 1) }", "n: Int", "\n  requires n >= 0", [[0], [1], [10]], ([n]) => 2 ** n],
    repeatDown: ["stars", "Return a text of n asterisks, built by recursion. n is not negative.", "fn stars(n: Int) -> Str\n  requires n >= 0\n  decreases n\n  = if n == 0 { \"\" } else { \"*\" ++ stars(n - 1) }", "n: Int", "\n  requires n >= 0", [[0], [3]], ([n]) => "*".repeat(n)],
    sumHalves: ["sumOfHalvings", "Return n + n/2 + n/4 + ... (integer division) down to 1. n is not negative.", "fn sumOfHalvings(n: Int) -> Int\n  requires n >= 0\n  decreases n\n  = if n == 0 { 0 } else { n + sumOfHalvings(n / 2) }", "n: Int", "\n  requires n >= 0", [[0], [1], [10], [100]], ([n]) => { let s = 0; for (let k = n; k > 0; k = Math.trunc(k / 2)) s += k; return s; }],
    lengthRec: ["countDownTo", "Return how many steps it takes to count down from n to 0 by subtracting 3 each time (the last step may overshoot). n is not negative.", "fn countDownTo(n: Int) -> Int\n  requires n >= 0\n  decreases n\n  = if n == 0 { 0 } else { 1 + countDownTo((n - 3).max(0)) }", "n: Int", "\n  requires n >= 0", [[0], [3], [4], [10]], ([n]) => Math.ceil(n / 3)],
    countUp: ["stepsToLimit", "Return how many times 1 can be added to start before reaching the limit (0 when start is already at the limit or above). Count by recursion on the distance.", "fn stepsToLimit(start: Int, limit: Int) -> Int\n  decreases limit - start\n  = if start >= limit { 0 } else { 1 + stepsToLimit(start + 1, limit) }", "start: Int, limit: Int", "", [[0, 5], [5, 5], [7, 3]], ([a, b]) => Math.max(b - a, 0)],
  }[kind];
  const nonNeg = kind === "lengthRec" || kind === "pow2" || kind === "repeatDown" || kind === "sumHalves" || kind === "halvings";
  void nonNeg;
  const ret = kind === "repeatDown" ? "Str" : "Int";
  return { name: spec[0], desc: spec[1], sig: `fn ${spec[0]}(${spec[3]}) -> ${ret}${spec[4]}`, code: spec[2], cases: spec[5].map((a) => [`${spec[0]}(${a.join(", ")})`, lit(spec[6](a))]) };
});

fam("proof.refined-params", (r) => {
  const kind = r.pick(["divideBy", "percentOfPositive", "everyNth", "repeatTimes"]);
  const spec = {
    divideBy: ["divideBy", "Divide a by d. The divisor type says it can never be zero.", "fn divideBy(a: Int, d: Int where it != 0) -> Int = a / d", "a: Int, d: Int where it != 0", "Int", [[9, 3], [-8, 2], [5, -5]], ([a, d]) => idiv(a, d)],
    percentOfPositive: ["fractionPercent", "Return part as a percent of total. The total type says it is positive.", "fn fractionPercent(part: Int, total: Int where it > 0) -> Int = part * 100 / total", "part: Int, total: Int where it > 0", "Int", [[1, 2], [3, 4], [0, 9]], ([a, t]) => idiv(a * 100, t)],
    everyNth: ["everyNth", "Return every n-th element of the list (positions 0, n, 2n, ...). n is positive.", "fn everyNth(xs: List[Int], n: Int where it > 0) -> List[Int] = xs.enumerate().filter(p => p.0 % n == 0).map(p => p.1)", "xs: List[Int], n: Int where it > 0", "List[Int]", [[[1, 2, 3, 4, 5], 2], [[1, 2, 3], 1], [[], 3]], ([l, n]) => l.filter((_, i) => i % n === 0)],
    repeatTimes: ["repeatList", "Return the list repeated n times, one copy after another. n is not negative (the type says so).", "fn repeatList(xs: List[Int], n: Int where it >= 0) -> List[Int] = List.repeat(xs, n).flatMap(l => l)", "xs: List[Int], n: Int where it >= 0", "List[Int]", [[[1, 2], 3], [[5], 0], [[], 4]], ([l, n]) => Array.from({ length: n }, () => l).flat()],
  }[kind];
  return { name: spec[0], desc: spec[1], sig: `fn ${spec[0]}(${spec[3]}) -> ${spec[4]}`, code: spec[2], cases: spec[5].map((a) => [`${spec[0]}(${a.map(lit).join(", ")})`, lit(spec[6](a))]) };
});
