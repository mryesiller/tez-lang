//! Differential tests: the compiled WebAssembly must agree with the reference interpreter on
//! every program. Each group is compiled to one module, validated with `wasm-tools` (when
//! installed), run in Node, and compared line by line.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use tez_interp::{Options, run_function, with_big_stack};
use tez_syntax::{ParseOptions, parse};
use tez_types::check;

fn node_available() -> bool {
    Command::new("node").arg("--version").output().is_ok()
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `(function name, expected)` from the interpreter, as the runner would print it.
fn interpret(prog: &tez_types::hir::Program, name: &str) -> String {
    match with_big_stack(|| run_function(prog, name, Options::default())) {
        Ok(v) => v,
        Err(e) => format!("ERROR {}", e.code.as_str()),
    }
}

fn run_wasm(prog: &tez_types::hir::Program, names: &[String], tag: &str) -> Vec<(String, String)> {
    let bytes = tez_wasm::compile(prog)
        .unwrap_or_else(|e| panic!("codegen failed: {e}"))
        .wasm;
    let dir = std::env::temp_dir().join(format!("tez-wasm-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{tag}.wasm"));
    std::fs::write(&file, &bytes).unwrap();
    if let Ok(out) = Command::new("wasm-tools")
        .args(["validate", "--features", "all"])
        .arg(&file)
        .output()
    {
        assert!(
            out.status.success(),
            "wasm-tools validate failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let out = Command::new("node")
        .arg(root().join("runtime/run.mjs"))
        .arg(&file)
        .args(names)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "node failed:\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(n, v)| (n.to_string(), v.to_string()))
        .collect()
}

fn compile(src: &str) -> tez_types::hir::Program {
    let parsed = parse(src, ParseOptions::default());
    assert!(
        parsed.problems.is_empty(),
        "parse: {:?}\n{src}",
        parsed.problems
    );
    let out = check(&parsed.file, src);
    assert!(out.problems.is_empty(), "check: {:#?}\n{src}", out.problems);
    out.program
}

fn agree(src: &str, names: &[String], tag: &str) {
    let prog = compile(src);
    let got = run_wasm(&prog, names, tag);
    assert_eq!(
        got.len(),
        names.len(),
        "runner printed {} lines for {} functions",
        got.len(),
        names.len()
    );
    for (name, value) in got {
        let want = interpret(&prog, &name);
        assert_eq!(
            value, want,
            "`{name}` differs (wasm vs interpreter)\n--- source ---\n{src}"
        );
    }
}

/// Each `(type, expr)` becomes `fn tN() -> type = expr`.
fn snippets(tag: &str, items: &[(&str, &str)]) {
    if !node_available() {
        eprintln!("node not installed: skipping");
        return;
    }
    let mut src = String::new();
    let mut names = Vec::new();
    for (i, (ty, e)) in items.iter().enumerate() {
        src.push_str(&format!("fn t{i}() -> {ty} = {e}\n"));
        names.push(format!("t{i}"));
    }
    agree(&src, &names, tag);
}

#[test]
fn arithmetic_and_big_integers() {
    snippets(
        "arith",
        &[
            ("Int", "1 + 2 * 3 - 4"),
            ("Int", "-7 / 2"),
            ("Int", "-7 % 2"),
            ("Int", "7 / -2"),
            ("Int", "-(3)"),
            ("Int", "1073741823 + 1"),
            ("Int", "0 - 1073741824 - 1"),
            ("Int", "1073741823 * 1073741823"),
            ("Int", "9223372036854775807 + 1"),
            ("Int", "9223372036854775807 * 9223372036854775807"),
            ("Int", "0 - 9223372036854775807 - 2"),
            ("Int", "2.pow(100)"),
            ("Int", "(2.pow(100) / 2.pow(98))"),
            ("Bool", "2.pow(100) - 2.pow(100) == 0"),
            ("Int", "2.pow(70) % 1000"),
            ("Bool", "2.pow(70) > 2.pow(69) && 3 < 2.pow(70)"),
            ("Int", "123456789012345678901234567890"),
            ("Int", "0 - 123456789012345678901234567890"),
            ("Bool", "1 < 2 && 2 <= 2 && 3 > 2 && 3 >= 3 && 1 != 2"),
            ("Float", "1.5 * 2.0"),
            ("Float", "7.0 / 2.0"),
            ("Float", "0.0 - 2.5"),
            ("Bool", "1.5 < 2.5 && 2.0 == 2.0"),
        ],
    );
}

#[test]
fn strings_and_conversions() {
    snippets(
        "strings",
        &[
            ("Str", "\"a\" ++ \"b\" ++ \"c\""),
            ("Int", "\"héllo\".len()"),
            ("Str", "\"hello\".slice(1, 3)"),
            ("Str", "\"  hi \".trim()"),
            ("Str", "\"abc\".upper()"),
            (
                "Bool",
                "\"abc\".contains(\"bc\") && \"abc\".startsWith(\"ab\") && \"abc\".endsWith(\"bc\")",
            ),
            ("List[Str]", "\"a,b,c\".split(\",\")"),
            ("Str", "[\"a\", \"b\"].join(\"-\")"),
            ("Option[Int]", "\"42\".toInt()"),
            ("Option[Int]", "\"-7\".toInt()"),
            ("Option[Int]", "\"4x\".toInt()"),
            ("Str", "\"ab\".repeat(3)"),
            ("List[Str]", "\"ab\".chars()"),
            ("Str", "42.toStr()"),
            ("Str", "1.5.toStr()"),
            ("Str", "2.0.toStr()"),
            (
                "Bool",
                "\"abc\" < \"abd\" && \"b\" > \"a\" && \"a\" <= \"a\"",
            ),
            ("Bool", "\"abc\" == \"abc\" && \"abc\" != \"abd\""),
            ("Int", "2.7.floor()"),
            ("Int", "2.1.ceil()"),
            ("Int", "2.5.round()"),
            ("Int", "(0.0 - 2.5).round()"),
            ("Float", "3.toFloat() / 2.0"),
            ("Float", "4.0.sqrt()"),
            ("Str", "\"quote\\\" and \\\\ and \\n\""),
        ],
    );
}

#[test]
fn lists_and_higher_order_functions() {
    snippets(
        "lists",
        &[
            ("List[Int]", "List.range(0, 5)"),
            ("List[Int]", "List.range(3, 1)"),
            ("List[Int]", "[1, 2, 3].map(x => x * 2)"),
            ("List[Int]", "List.range(0, 10).filter(x => x % 2 == 0)"),
            ("Int", "[1, 2, 3, 4].fold(0, (a, b) => a + b)"),
            ("Int", "[1, 2, 3].sum()"),
            ("List[Int]", "[1, 2].append(3)"),
            ("List[Int]", "[1, 2].concat([3])"),
            ("List[Int]", "[1, 2, 3].reverse()"),
            ("List[Int]", "[1, 2, 3].take(2)"),
            ("List[Int]", "[1, 2, 3].drop(2)"),
            ("List[Int]", "[1, 2, 3].take(0 - 1)"),
            ("List[Int]", "[1, 2, 3].drop(9)"),
            ("Bool", "[1, 2].any(x => x > 1) && [1, 2].all(x => x > 0)"),
            ("Int", "[1, 2, 3].count(x => x > 1)"),
            ("Bool", "[1, 2].contains(2)"),
            ("Option[Int]", "[5, 6].indexOf(6)"),
            ("Option[Int]", "[5, 6].first()"),
            ("Option[Int]", "[5, 6].last()"),
            ("Option[Int]", "[].first()"),
            ("List[(Int, Str)]", "[1, 2].zip([\"a\", \"b\", \"c\"])"),
            ("List[(Int, Int)]", "[7, 8].enumerate()"),
            ("List[Int]", "[3, 1, 2].sort()"),
            ("List[Str]", "[\"bb\", \"a\", \"ccc\"].sortBy(s => s.len())"),
            ("Option[Int]", "[3, 1, 2].min()"),
            ("Option[Int]", "[3, 1, 2].max()"),
            ("List[Int]", "[1, 2].flatMap(x => [x, x])"),
            ("List[Int]", "List.repeat(7, 3)"),
            ("Int", "[10, 20, 30][1]"),
            ("Int", "[10, 20].get(0)"),
            ("Bool", "List.range(0, 0).isEmpty()"),
            (
                "Bool",
                "[1, 2, 3] == [1, 2, 3] && [1] != [2] && [[1], [2]] == [[1], [2]]",
            ),
            ("List[Int]", "List.range(0, 3).map(x => 2.pow(40) + x)"),
            ("Int", "[2.pow(40), 2.pow(41)].sum()"),
        ],
    );
}

#[test]
fn option_result_tuples_and_equality() {
    snippets(
        "adts",
        &[
            ("Int", "Some(3).unwrapOr(0) + None.unwrapOr(10)"),
            ("Option[Int]", "Some(3).map(x => x + 1)"),
            (
                "Option[Int]",
                "Some(3).andThen(x => if x > 5 { Some(x) } else { None })",
            ),
            ("Bool", "Some(1).isSome() && !Some(2).isNone()"),
            ("Result[Int, Str]", "Ok(1).map(x => x + 1)"),
            ("Result[Int, Int]", "Err(\"a\").mapErr(s => s.len())"),
            ("Int", "Err(\"a\").unwrapOr(7)"),
            ("(Int, Str, Bool)", "(1, \"x\", true)"),
            ("Int", "(1, (2, 3)).1.0"),
            (
                "Bool",
                "Some(1) == Some(1) && Some(1) != None && Some(Some(2)) == Some(Some(2))",
            ),
            (
                "Bool",
                "(1, \"a\") == (1, \"a\") && (1, \"a\") != (2, \"a\")",
            ),
            ("Option[List[Int]]", "Some([1, 2])"),
            ("Unit", "()"),
        ],
    );
}

#[test]
fn programs_with_types_matching_and_closures() {
    if !node_available() {
        return;
    }
    let src = "type Point = { x: Int, y: Int }
type Shape = Circle(Int) | Rect(Int, Int) | Empty
type Tree = Leaf | Node(Tree, Int, Tree)
fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Rect(w, h) => w * h
  Empty => 0
}
fn insert(t: Tree, v: Int) -> Tree
  decreases t
  = match t {
  Leaf => Node(Leaf, v, Leaf)
  Node(l, x, r) => if v < x { Node(insert(l, v), x, r) } else { Node(l, x, insert(r, v)) }
}
fn sumTree(t: Tree) -> Int
  decreases t
  = match t {
  Leaf => 0
  Node(l, x, r) => sumTree(l) + x + sumTree(r)
}
fn len(xs: List[Int]) -> Int
  decreases xs.len()
  = match xs {
  [] => 0
  [_, ..t] => 1 + len(t)
}
fn adder(n: Int) -> (Int) -> Int = x => x + n
fn compose(f: (Int) -> Int, g: (Int) -> Int) -> (Int) -> Int = x => g(f(x))
fn classify(n: Int) -> Str = match n {
  0 => \"zero\"
  x if x < 0 => \"neg\"
  _ => \"pos\"
}
fn first(o: Option[Option[Int]]) -> Int = match o {
  Some(Some(n)) => n
  Some(None) => -1
  None => -2
}
fn lists(xs: List[Int]) -> Str = match xs {
  [] => \"empty\"
  [a] => \"one\"
  [a, b] => \"two\"
  [a, ..rest] => \"many\"
}
const base: Int = 10
const doubled: Int = base * 2
fn records() -> (Point, Point) = {
  let p = Point { x: 1, y: 2 }
  (p, p with { y: 9 })
}
fn apply2() -> Int = compose(adder(5), adder(2))(10)
fn shapes() -> (Int, Int) = (area(Rect(3, 4)) + area(Circle(2)) + area(Empty), len([1, 2, 3, 4]))
fn tree() -> Int = sumTree(insert(insert(insert(Leaf, 5), 2), 9))
fn guards() -> (Str, Str, Str) = (classify(0), classify(-3), classify(8))
fn nested() -> (Int, Int, Int) = (first(Some(Some(4))), first(Some(None)), first(None))
fn listpat() -> List[Str] = [lists([]), lists([1]), lists([1, 2]), lists([1, 2, 3])]
fn consts() -> Int = doubled + base
fn blocks() -> Int = {
  let a = base + 1
  let (b, c) = (a, doubled)
  assert b < c
  b + c
}
fn capture() -> List[Int] = {
  let k = 100
  let big = 2.pow(40)
  List.range(0, 3).map(x => x + k).map(y => y + big).map(z => z - big)
}
fn fnvalue() -> List[Int] = List.range(0, 4).map(adder(1))
fn bigrec() -> Int = if len(List.range(0, 3000)) > 2999 { 1 } else { 0 }
";
    let names: Vec<String> = [
        "records", "apply2", "shapes", "tree", "guards", "nested", "listpat", "consts", "blocks",
        "capture", "fnvalue", "bigrec",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    agree(src, &names, "program");
}

#[test]
fn runtime_failures_agree() {
    if !node_available() {
        return;
    }
    let src = "fn d() -> Int = 1 / 0
fn m() -> Int = 1 % 0
fn idx() -> Int = [1, 2][5]
fn sl() -> Str = \"abc\".slice(2, 9)
fn sp() -> List[Str] = \"abc\".split(\"\")
fn big() -> Int = 2.pow(100) / (2.pow(100) - 2.pow(100))
fn asrt() -> Int = {
  assert 1 > 2
  1
}
";
    let names: Vec<String> = ["d", "m", "idx", "sl", "sp", "big", "asrt"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    agree(src, &names, "failures");
}

#[test]
fn every_example_program_agrees() {
    if !node_available() {
        return;
    }
    let dir = root().join("spec/examples");
    let mut n = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if !name.ends_with(".tez") || name.ends_with(".reject.tez") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        agree(&src, &["main".to_string()], &name.replace('.', "_"));
        n += 1;
    }
    assert!(n >= 14, "found only {n} examples");
}
