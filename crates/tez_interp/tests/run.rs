//! End-to-end evaluation tests: source text in, rendered value or error code out.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use tez_diag::Code;
use tez_interp::{Options, run_function, run_tests, with_big_stack};
use tez_syntax::{ParseOptions, parse};
use tez_types::check;

fn compile(src: &str) -> tez_types::hir::Program {
    let parsed = parse(src, ParseOptions::default());
    assert!(parsed.problems.is_empty(), "parse: {:?}", parsed.problems);
    let out = check(&parsed.file, src);
    assert!(out.problems.is_empty(), "check: {:#?}\n{src}", out.problems);
    out.program
}

fn run_opts(src: &str, opts: Options) -> Result<String, Code> {
    let prog = compile(src);
    with_big_stack(|| run_function(&prog, "main", opts)).map_err(|e| e.code)
}

fn run(src: &str) -> String {
    run_opts(src, Options::default()).unwrap_or_else(|c| panic!("runtime error {c:?}\n{src}"))
}

/// Evaluates `expr`, declared with return type `ty`.
fn ev(ty: &str, expr: &str) -> String {
    run(&format!("fn main() -> {ty} = {expr}"))
}

fn err(ty: &str, expr: &str) -> Code {
    run_opts(&format!("fn main() -> {ty} = {expr}"), Options::default()).unwrap_err()
}

#[test]
fn arithmetic_and_precedence() {
    assert_eq!(ev("Int", "1 + 2 * 3 - 4"), "3");
    assert_eq!(ev("Int", "-7 / 2"), "-3");
    assert_eq!(ev("Int", "-7 % 2"), "-1");
    assert_eq!(ev("Int", "7 / -2"), "-3");
    assert_eq!(ev("Float", "1.5 * 2.0"), "3.0");
    assert_eq!(ev("Float", "7.0 / 2.0"), "3.5");
    assert_eq!(
        ev("Bool", "1 < 2 && 2 <= 2 && 3 > 2 && 3 >= 3 && 1 != 2"),
        "true"
    );
    assert_eq!(ev("Str", "\"a\" ++ \"b\" ++ \"c\""), "\"abc\"");
    assert_eq!(ev("Bool", "!false"), "true");
    assert_eq!(ev("Int", "-(3)"), "-3");
}

#[test]
fn arbitrary_precision_integers() {
    assert_eq!(ev("Int", "9223372036854775807 + 1"), "9223372036854775808");
    assert_eq!(
        ev("Int", "9223372036854775807 * 9223372036854775807"),
        "85070591730234615847396907784232501249"
    );
    assert_eq!(
        ev("Int", "0 - 9223372036854775807 - 2"),
        "-9223372036854775809"
    );
    assert_eq!(ev("Int", "2.pow(100)"), "1267650600228229401496703205376");
    assert_eq!(ev("Int", "(2.pow(100) / 2.pow(98))"), "4");
    assert_eq!(ev("Bool", "2.pow(100) - 2.pow(100) == 0"), "true");
    assert_eq!(
        run(
            "fn fact(n: Int) -> Int\n  decreases n\n  = if n <= 1 { 1 } else { n * fact(n - 1) }\nfn main() -> Int = fact(25)"
        ),
        "15511210043330985984000000"
    );
}

#[test]
fn division_by_zero_and_float_domain() {
    assert_eq!(err("Int", "1 / 0"), Code::DivZero);
    assert_eq!(err("Int", "1 % 0"), Code::DivZero);
    assert_eq!(err("Float", "1.0 / 0.0"), Code::FloatDomain);
    assert_eq!(err("Float", "(0.0 - 1.0).sqrt()"), Code::FloatDomain);
    assert_eq!(ev("Float", "4.0.sqrt()"), "2.0");
}

#[test]
fn strings() {
    assert_eq!(ev("Int", "\"héllo\".len()"), "5");
    assert_eq!(ev("Str", "\"hello\".slice(1, 3)"), "\"el\"");
    assert_eq!(ev("Str", "\"  hi \".trim()"), "\"hi\"");
    assert_eq!(ev("Str", "\"abc\".upper()"), "\"ABC\"");
    assert_eq!(
        ev(
            "Bool",
            "\"abc\".contains(\"bc\") && \"abc\".startsWith(\"ab\") && \"abc\".endsWith(\"bc\")"
        ),
        "true"
    );
    assert_eq!(
        ev("List[Str]", "\"a,b,c\".split(\",\")"),
        "[\"a\", \"b\", \"c\"]"
    );
    assert_eq!(ev("Str", "[\"a\", \"b\"].join(\"-\")"), "\"a-b\"");
    assert_eq!(ev("Option[Int]", "\"42\".toInt()"), "Some(42)");
    assert_eq!(ev("Option[Int]", "\"-7\".toInt()"), "Some(-7)");
    assert_eq!(ev("Option[Int]", "\"4x\".toInt()"), "None");
    assert_eq!(ev("Option[Int]", "\"\".toInt()"), "None");
    assert_eq!(ev("Str", "\"ab\".repeat(3)"), "\"ababab\"");
    assert_eq!(ev("List[Str]", "\"ab\".chars()"), "[\"a\", \"b\"]");
    assert_eq!(ev("Str", "42.toStr()"), "\"42\"");
    assert_eq!(ev("Str", "1.5.toStr()"), "\"1.5\"");
    assert_eq!(ev("Str", "2.0.toStr()"), "\"2.0\"");
    assert_eq!(err("Str", "\"abc\".slice(2, 9)"), Code::Precondition);
    assert_eq!(err("List[Str]", "\"abc\".split(\"\")"), Code::Precondition);
}

#[test]
fn floats_and_conversions() {
    assert_eq!(ev("Int", "2.7.floor()"), "2");
    assert_eq!(ev("Int", "2.1.ceil()"), "3");
    assert_eq!(ev("Int", "2.5.round()"), "3");
    assert_eq!(ev("Int", "(0.0 - 2.5).round()"), "-3");
    assert_eq!(ev("Float", "3.toFloat() / 2.0"), "1.5");
    assert_eq!(ev("Float", "(0.0 - 1.5).abs()"), "1.5");
    assert_eq!(ev("Int", "(0 - 5).abs()"), "5");
    assert_eq!(ev("Int", "3.min(5) + 3.max(5)"), "8");
    assert_eq!(ev("Int", "15.clamp(0, 10)"), "10");
    assert_eq!(ev("Bool", "4.isEven() && 3.isOdd()"), "true");
}

#[test]
fn lists_and_higher_order() {
    assert_eq!(ev("List[Int]", "List.range(0, 5)"), "[0, 1, 2, 3, 4]");
    assert_eq!(ev("List[Int]", "List.range(3, 1)"), "[]");
    assert_eq!(ev("List[Int]", "[1, 2, 3].map(x => x * 2)"), "[2, 4, 6]");
    assert_eq!(
        ev("List[Int]", "List.range(0, 10).filter(x => x % 2 == 0)"),
        "[0, 2, 4, 6, 8]"
    );
    assert_eq!(ev("Int", "[1, 2, 3, 4].fold(0, (a, b) => a + b)"), "10");
    assert_eq!(ev("Int", "[1, 2, 3].sum()"), "6");
    assert_eq!(ev("List[Int]", "[1, 2].append(3)"), "[1, 2, 3]");
    assert_eq!(ev("List[Int]", "[1, 2].concat([3])"), "[1, 2, 3]");
    assert_eq!(ev("List[Int]", "[1, 2, 3].reverse()"), "[3, 2, 1]");
    assert_eq!(ev("List[Int]", "[1, 2, 3].take(2)"), "[1, 2]");
    assert_eq!(ev("List[Int]", "[1, 2, 3].drop(2)"), "[3]");
    assert_eq!(ev("List[Int]", "[1, 2, 3].take(0 - 1)"), "[]");
    assert_eq!(ev("List[Int]", "[1, 2, 3].drop(9)"), "[]");
    assert_eq!(
        ev("Bool", "[1, 2].any(x => x > 1) && [1, 2].all(x => x > 0)"),
        "true"
    );
    assert_eq!(ev("Int", "[1, 2, 3].count(x => x > 1)"), "2");
    assert_eq!(ev("Bool", "[1, 2].contains(2)"), "true");
    assert_eq!(ev("Option[Int]", "[5, 6].indexOf(6)"), "Some(1)");
    assert_eq!(ev("Option[Int]", "[5, 6].first()"), "Some(5)");
    assert_eq!(ev("Option[Int]", "[5, 6].last()"), "Some(6)");
    assert_eq!(ev("Option[Int]", "[].first()"), "None");
    assert_eq!(
        ev("List[(Int, Str)]", "[1, 2].zip([\"a\", \"b\", \"c\"])"),
        "[(1, \"a\"), (2, \"b\")]"
    );
    assert_eq!(
        ev("List[(Int, Int)]", "[7, 8].enumerate()"),
        "[(0, 7), (1, 8)]"
    );
    assert_eq!(ev("List[Int]", "[3, 1, 2].sort()"), "[1, 2, 3]");
    assert_eq!(
        ev("List[Str]", "[\"bb\", \"a\", \"ccc\"].sortBy(s => s.len())"),
        "[\"a\", \"bb\", \"ccc\"]"
    );
    assert_eq!(ev("Option[Int]", "[3, 1, 2].min()"), "Some(1)");
    assert_eq!(ev("Option[Int]", "[3, 1, 2].max()"), "Some(3)");
    assert_eq!(
        ev("List[Int]", "[1, 2].flatMap(x => [x, x])"),
        "[1, 1, 2, 2]"
    );
    assert_eq!(ev("List[Int]", "List.repeat(7, 3)"), "[7, 7, 7]");
    assert_eq!(ev("Int", "[10, 20, 30][1]"), "20");
    assert_eq!(ev("Int", "[10, 20].get(0)"), "10");
    assert_eq!(ev("Bool", "List.range(0, 0).isEmpty()"), "true");
    assert_eq!(err("Int", "[1, 2][5]"), Code::IndexBounds);
    assert_eq!(err("Int", "[1, 2].get(0 - 1)"), Code::IndexBounds);
}

#[test]
fn records_adts_and_match() {
    let src = "type Point = { x: Int, y: Int }
type Shape = Circle(Int) | Rect(Int, Int) | Empty
fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Rect(w, h) => w * h
  Empty => 0
}
fn main() -> (Int, Point, Point) = {
  let p = Point { x: 1, y: 2 }
  (area(Rect(3, 4)) + area(Circle(2)) + area(Empty), p, p with { y: 9 })
}";
    assert_eq!(run(src), "(24, Point { x: 1, y: 2 }, Point { x: 1, y: 9 })");
}

#[test]
fn recursive_types_and_lists_by_pattern() {
    let src = "type Tree = Leaf | Node(Tree, Int, Tree)
fn insert(t: Tree, v: Int) -> Tree
  decreases 0
  = match t {
  Leaf => Node(Leaf, v, Leaf)
  Node(l, x, r) => if v < x { Node(insert(l, v), x, r) } else { Node(l, x, insert(r, v)) }
}
fn sum(t: Tree) -> Int
  decreases 0
  = match t {
  Leaf => 0
  Node(l, x, r) => sum(l) + x + sum(r)
}
fn len(xs: List[Int]) -> Int
  decreases xs.len()
  = match xs {
  [] => 0
  [_, ..t] => 1 + len(t)
}
fn main() -> (Int, Int) = (sum(insert(insert(insert(Leaf, 5), 2), 9)), len([1, 2, 3, 4]))";
    assert_eq!(run(src), "(16, 4)");
}

#[test]
fn closures_capture_their_environment() {
    let src = "fn adder(n: Int) -> (Int) -> Int = x => x + n
fn compose(f: (Int) -> Int, g: (Int) -> Int) -> (Int) -> Int = x => g(f(x))
fn main() -> Int = {
  let add5 = adder(5)
  let add2 = adder(2)
  compose(add5, add2)(10)
}";
    assert_eq!(run(src), "17");
}

#[test]
fn guards_literals_and_nested_patterns() {
    let src = "fn classify(n: Int) -> Str = match n {
  0 => \"zero\"
  x if x < 0 => \"neg\"
  _ => \"pos\"
}
fn first(o: Option[Option[Int]]) -> Int = match o {
  Some(Some(n)) => n
  Some(None) => -1
  None => -2
}
fn main() -> (Str, Str, Str, Int, Int, Int) =
  (classify(0), classify(-3), classify(8), first(Some(Some(4))), first(Some(None)), first(None))";
    assert_eq!(run(src), "(\"zero\", \"neg\", \"pos\", 4, -1, -2)");
}

#[test]
fn list_patterns() {
    let src = "fn f(xs: List[Int]) -> Str = match xs {
  [] => \"empty\"
  [a] => \"one\"
  [a, b] => \"two\"
  [a, ..rest] => \"many\"
}
fn main() -> List[Str] = [f([]), f([1]), f([1, 2]), f([1, 2, 3])]";
    assert_eq!(run(src), "[\"empty\", \"one\", \"two\", \"many\"]");
}

#[test]
fn option_and_result_methods() {
    assert_eq!(ev("Int", "Some(3).unwrapOr(0) + None.unwrapOr(10)"), "13");
    assert_eq!(ev("Option[Int]", "Some(3).map(x => x + 1)"), "Some(4)");
    assert_eq!(
        ev(
            "Option[Int]",
            "Some(3).andThen(x => if x > 5 { Some(x) } else { None })"
        ),
        "None"
    );
    assert_eq!(ev("Bool", "Some(1).isSome() && !Some(2).isNone()"), "true");
    assert_eq!(ev("Result[Int, Str]", "Ok(1).map(x => x + 1)"), "Ok(2)");
    assert_eq!(
        ev("Result[Int, Int]", "Err(\"a\").mapErr(s => s.len())"),
        "Err(1)"
    );
    assert_eq!(ev("Int", "Err(\"a\").unwrapOr(7)"), "7");
    assert_eq!(
        ev(
            "Option[Int]",
            "{\n  let r: Result[Int, Str] = Ok(2)\n  r.toOption()\n}"
        ),
        "Some(2)"
    );
    assert_eq!(
        ev(
            "Bool",
            "{\n  let r: Result[Int, Str] = Ok(2)\n  r.isOk()\n}"
        ),
        "true"
    );
}

#[test]
fn constants_and_blocks() {
    let src = "const base: Int = 10
const doubled: Int = base * 2
fn main() -> Int = {
  let a = base + 1
  let (b, c) = (a, doubled)
  assert b < c
  b + c
}";
    assert_eq!(run(src), "31");
}

#[test]
fn asserts_fail_at_runtime() {
    assert_eq!(err("Int", "{\n  assert 1 > 2\n  1\n}"), Code::Assert);
}

#[test]
fn resource_limits() {
    let deep = "fn down(n: Int) -> Int\n  decreases n\n  = if n == 0 { 0 } else { 1 + down(n - 1) }\nfn main() -> Int = down(1000000)";
    assert_eq!(
        run_opts(
            deep,
            Options {
                max_depth: 1000,
                ..Options::default()
            }
        )
        .unwrap_err(),
        Code::Resource
    );
    assert_eq!(
        run_opts(deep, Options::default()).unwrap_err(),
        Code::Resource
    );
    let ok = "fn down(n: Int) -> Int\n  decreases n\n  = if n == 0 { 0 } else { 1 + down(n - 1) }\nfn main() -> Int = down(10000)";
    assert_eq!(run(ok), "10000");
    let spin = "fn spin(n: Int) -> Int\n  decreases n\n  = if n == 0 { 0 } else { spin(n - 1) }\nfn main() -> Int = spin(100000)";
    assert_eq!(
        run_opts(
            spin,
            Options {
                fuel: 1000,
                ..Options::default()
            }
        )
        .unwrap_err(),
        Code::Resource
    );
    assert_eq!(
        err("List[Int]", "List.range(0, 100000000000)"),
        Code::Resource
    );
}

#[test]
fn contract_checking_mode() {
    let on = Options {
        check_contracts: true,
        ..Options::default()
    };
    let src = "fn div(a: Int, b: Int) -> Int requires b != 0 = a / b\nfn main() -> Int = div(1, 0)";
    assert_eq!(run_opts(src, on.clone()).unwrap_err(), Code::Precondition);
    assert_eq!(
        run_opts(src, Options::default()).unwrap_err(),
        Code::DivZero
    );
    let post = "fn bad(x: Int) -> Int ensures result > 0 = x\nfn main() -> Int = bad(0)";
    assert_eq!(run_opts(post, on.clone()).unwrap_err(), Code::Postcondition);
    assert_eq!(run(post), "0");
    let refine = "fn f(x: Int where it > 0) -> Int = x\nfn main() -> Int = f(0)";
    assert_eq!(run_opts(refine, on.clone()).unwrap_err(), Code::Refinement);
    let field = "type P = { n: Int where it > 0 }\nfn main() -> P = P { n: 0 }";
    assert_eq!(run_opts(field, on.clone()).unwrap_err(), Code::Refinement);
    let ok = "fn f(x: Int where it > 0) -> Int where it > 1 = x + 1\nfn main() -> Int = f(1)";
    assert_eq!(run_opts(ok, on).unwrap(), "2");
}

#[test]
fn tests_are_run() {
    let prog = compile(
        "fn double(x: Int) -> Int = x * 2\ntest ok1 = double(2) == 4\ntest bad = double(2) == 5\ntest boom = 1 / 0 == 0",
    );
    let results = with_big_stack(|| run_tests(&prog, &Options::default()));
    assert_eq!(results.len(), 3);
    assert!(matches!(results[0].outcome, Ok(true)));
    assert!(matches!(results[1].outcome, Ok(false)));
    assert_eq!(results[2].outcome.as_ref().unwrap_err().code, Code::DivZero);
}

#[test]
fn display_of_values() {
    assert_eq!(ev("Unit", "()"), "()");
    assert_eq!(ev("(Bool, Float)", "(true, 1.0)"), "(true, 1.0)");
    assert_eq!(ev("Option[List[Int]]", "Some([1])"), "Some([1])");
}

#[test]
fn float_overflow_stops_the_program_instead_of_producing_infinity_or_nan() {
    let big = format!("1{}.0", "0".repeat(200));
    assert_eq!(err("Float", &format!("{big} * {big}")), Code::Resource);
    assert_eq!(
        err("Float", &format!("{big} * {big} - {big} * {big}")),
        Code::Resource
    );
    let tiny = format!("0.{}1", "0".repeat(200));
    assert_eq!(err("Float", &format!("{big} / {tiny}")), Code::Resource);
    assert_eq!(ev("Float", "1.5 * 2.0"), "3.0");
}

#[test]
fn case_mapping_can_change_the_length() {
    // the reason Str.upper/lower carry no length contract
    assert_eq!(ev("Int", "\"ß\".upper().len()"), "2");
    assert_eq!(ev("Int", "\"İ\".lower().len()"), "2");
    assert_eq!(ev("Int", "\"😀é\".len()"), "2");
    assert_eq!(ev("Int", "\"😀é\".chars().len()"), "2");
}

#[test]
fn trusted_prelude_contracts_hold_on_hard_inputs() {
    let src = include_str!("prelude_contracts.tez");
    let prog = compile(src);
    let results = with_big_stack(|| run_tests(&prog, &Options::default()));
    assert!(results.len() >= 16, "the contract tests went missing");
    for r in &results {
        assert!(
            matches!(r.outcome, Ok(true)),
            "prelude contract `{}` is violated: {r:?}",
            r.name
        );
    }
}
