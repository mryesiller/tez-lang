//! Verifier tests. "Proved" programs must be accepted; every violation class must be refused
//! with its own code. Skipped (with a note) when `z3` is not installed.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use tez_diag::Code;
use tez_syntax::{ParseOptions, Problem, parse};
use tez_types::check;
use tez_verify::{VerifyOptions, verify};

fn z3_available() -> bool {
    std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_ok()
}

fn run(src: &str) -> Vec<Problem> {
    let parsed = parse(src, ParseOptions::default());
    assert!(parsed.problems.is_empty(), "parse: {:?}", parsed.problems);
    let out = check(&parsed.file, src);
    assert!(out.problems.is_empty(), "check: {:#?}\n{src}", out.problems);
    verify(
        &out.program,
        src,
        &VerifyOptions {
            timeout_ms: 6000,
            include_prelude: false,
            cache_dir: None,
            backend: tez_verify::Backend::Process,
        },
    )
    .unwrap()
}

macro_rules! need_z3 {
    () => {
        if !z3_available() {
            eprintln!("z3 not installed: skipping");
            return;
        }
    };
}

fn proves(src: &str) {
    need_z3!();
    let ps = run(src);
    assert!(
        ps.is_empty(),
        "expected the program to verify, got:\n{:#?}\nsource:\n{src}",
        ps.iter()
            .map(|p| (p.code, &p.message, &p.counterexample))
            .collect::<Vec<_>>()
    );
}

fn rejects(src: &str, code: Code) {
    need_z3!();
    let ps = run(src);
    assert!(
        ps.iter().any(|p| p.code == code),
        "expected {code:?}, got {:?}\nsource:\n{src}",
        ps.iter().map(|p| (p.code, &p.message)).collect::<Vec<_>>()
    );
}

#[test]
fn prelude_function_bodies_satisfy_their_contracts() {
    need_z3!();
    let parsed = parse("", ParseOptions::default());
    let out = check(&parsed.file, "");
    let ps = verify(
        &out.program,
        "",
        &VerifyOptions {
            timeout_ms: 8000,
            include_prelude: true,
            cache_dir: None,
            backend: tez_verify::Backend::Process,
        },
    )
    .unwrap();
    assert!(
        ps.is_empty(),
        "prelude verification failed: {:#?}",
        ps.iter().map(|p| (p.code, &p.message)).collect::<Vec<_>>()
    );
}

#[test]
fn division() {
    proves("fn f(a: Int, b: Int) -> Int requires b != 0 = a / b");
    proves("fn f(a: Int, b: Int where it != 0) -> Int = a / b");
    proves("fn f(a: Int, b: Int) -> Int = if b == 0 { 0 } else { a / b }");
    proves("fn f(a: Int, b: Int) -> Int = if b > 0 && a > 0 { a % b } else { 0 }");
    rejects("fn f(a: Int, b: Int) -> Int = a / b", Code::DivZero);
    rejects("fn f(a: Int, b: Int) -> Int = a % b", Code::DivZero);
    rejects(
        "fn f(a: Int, b: Int) -> Int requires b >= 0 = a / b",
        Code::DivZero,
    );
    rejects(
        "fn f(a: Int, b: Int) -> Int = if b != 0 || a > 0 { a / b } else { 0 }",
        Code::DivZero,
    );
}

#[test]
fn division_truncates_toward_zero() {
    proves("fn f() -> Int ensures result == -3 = -7 / 2");
    rejects(
        "fn f() -> Int ensures result == -4 = -7 / 2",
        Code::Postcondition,
    );
    proves("fn f() -> Int ensures result == -1 = -7 % 2");
    proves("fn f(a: Int) -> Int ensures result * 2 <= a + 1 && result * 2 >= a - 1 = a / 2");
    // would hold for floor division, must fail for truncation
    rejects(
        "fn f(a: Int) -> Int ensures result * 2 <= a = a / 2",
        Code::Postcondition,
    );
}

#[test]
fn postconditions() {
    proves(
        "fn clamp(x: Int, lo: Int, hi: Int) -> Int requires lo <= hi ensures lo <= result && result <= hi = if x < lo { lo } else if x > hi { hi } else { x }",
    );
    proves("fn abs(x: Int) -> Int ensures result >= 0 = if x < 0 { 0 - x } else { x }");
    rejects(
        "fn f(x: Int) -> Int ensures result > x = x",
        Code::Postcondition,
    );
    rejects(
        "fn f(x: Int) -> Int ensures result >= 0 = x",
        Code::Postcondition,
    );
    rejects(
        "fn f(x: Int) -> Int ensures result == x + 1 = x + 2",
        Code::Postcondition,
    );
    proves("fn f(x: Int) -> Int ensures result == x + 1 ensures result > x = x + 1");
}

#[test]
fn indexing() {
    proves("fn f(xs: List[Int]) -> Int requires xs.len() > 0 = xs[0]");
    proves("fn f(xs: List[Int], i: Int) -> Int = if 0 <= i && i < xs.len() { xs[i] } else { 0 }");
    proves("fn f(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [h, ..t] => xs[0] + h\n}");
    rejects("fn f(xs: List[Int]) -> Int = xs[0]", Code::IndexBounds);
    rejects(
        "fn f(xs: List[Int], i: Int) -> Int requires i >= 0 = xs[i]",
        Code::IndexBounds,
    );
    rejects(
        "fn f(xs: List[Int], i: Int) -> Int requires i < xs.len() = xs[i]",
        Code::IndexBounds,
    );
    rejects(
        "fn f(xs: List[Int]) -> Int = xs.get(xs.len())",
        Code::IndexBounds,
    );
    proves("fn f(xs: List[Int]) -> Int requires xs.len() > 1 = xs[xs.len() - 1] + xs[1]");
}

#[test]
fn list_facts() {
    proves("fn f(xs: List[Int]) -> List[Int] ensures result.len() == xs.len() + 1 = xs.append(1)");
    proves("fn f(xs: List[Int], x: Int) -> Int ensures result == x = xs.append(x)[xs.len()]");
    proves("fn f() -> Int ensures result == 3 = [1, 2, 3].len()");
    proves("fn f() -> Int ensures result == 20 = [10, 20, 30][1]");
    proves(
        "fn f(n: Int) -> List[Int] requires n >= 0 ensures result.len() == n = List.range(0, n)",
    );
    proves("fn f(n: Int) -> Int requires n > 2 ensures result == 2 = List.range(0, n)[2]");
    rejects(
        "fn f(xs: List[Int]) -> Int ensures result > 0 = xs.len()",
        Code::Postcondition,
    );
    proves("fn f(xs: List[Int]) -> Int ensures result >= 0 = xs.len()");
    proves(
        "fn f(xs: List[Int], ys: List[Int]) -> Int ensures result == xs.len() + ys.len() = xs.concat(ys).len()",
    );
    proves("fn f(xs: List[Int]) -> Int ensures result <= xs.len() = xs.take(3).len()");
    rejects(
        "fn f(xs: List[Int]) -> Int ensures result == 3 = xs.take(3).len()",
        Code::Postcondition,
    );
    proves(
        "fn f(xs: List[Int]) -> List[Int] ensures result.len() == xs.len() = xs.map(x => x + 1)",
    );
    proves(
        "fn f(xs: List[Int]) -> List[Int] ensures result.len() <= xs.len() = xs.filter(x => x > 0)",
    );
}

#[test]
fn preconditions_at_call_sites() {
    let g = "fn g(x: Int) -> Int requires x > 0 = x\n";
    proves(&format!("{g}fn f() -> Int = g(5)"));
    proves(&format!(
        "{g}fn f(y: Int) -> Int = if y > 0 {{ g(y) }} else {{ 0 }}"
    ));
    proves(&format!("{g}fn f(y: Int where it > 3) -> Int = g(y)"));
    rejects(&format!("{g}fn f() -> Int = g(0)"), Code::Precondition);
    rejects(
        &format!("{g}fn f(y: Int) -> Int = g(y)"),
        Code::Precondition,
    );
    rejects(
        &format!("{g}fn f(y: Int) -> Int = g(y - 1)"),
        Code::Precondition,
    );
    proves(&format!("{g}fn f(y: Int) -> Int requires y > 5 = g(y - 1)"));
}

#[test]
fn contracts_of_callees_are_used() {
    proves(
        "fn pos(x: Int) -> Int ensures result > 0 = if x > 0 { x } else { 1 }\nfn f(x: Int) -> Int = 100 / pos(x)",
    );
    rejects(
        "fn nz(x: Int) -> Int = if x > 0 { x } else { 0 }\nfn f(x: Int) -> Int = 100 / nz(x)",
        Code::DivZero,
    );
    // small non-recursive callees are unfolded
    proves("fn double(x: Int) -> Int = x * 2\nfn f() -> Int = 10 / double(3)");
    proves("fn inc(x: Int) -> Int = x + 1\nfn f(x: Int) -> Int requires x >= 0 = 10 / inc(x)");
}

#[test]
fn refined_types() {
    proves("fn f(x: Int where it > 0) -> Int where it > 1 = x + 1");
    rejects(
        "fn f(x: Int where it > 0) -> Int where it > 5 = x + 1",
        Code::Refinement,
    );
    proves("type Pos = Int where it > 0\nfn g(x: Pos) -> Int = x\nfn f() -> Int = g(3)");
    rejects(
        "type Pos = Int where it > 0\nfn g(x: Pos) -> Int = x\nfn f(y: Int) -> Int = g(y)",
        Code::Refinement,
    );
    proves("type Pos = Int where it > 0\nfn div(a: Int, b: Pos) -> Int = a / b");
    proves("fn f(x: Int) -> Int = {\n  let y: Int where it > 0 = x * x + 1\n  100 / y\n}");
    rejects(
        "fn f(x: Int) -> Int = {\n  let y: Int where it > 0 = x\n  y\n}",
        Code::Refinement,
    );
    proves("fn f(lo: Int, hi: Int where it >= lo) -> Int ensures result >= 0 = hi - lo");
}

#[test]
fn refined_fields() {
    let t = "type Acct = { balance: Int where it >= 0 }\n";
    proves(&format!("{t}fn open() -> Acct = Acct {{ balance: 0 }}"));
    proves(&format!(
        "{t}fn dep(a: Acct, n: Int where it > 0) -> Acct = a with {{ balance: a.balance + n }}"
    ));
    rejects(
        &format!("{t}fn bad() -> Acct = Acct {{ balance: -1 }}"),
        Code::Refinement,
    );
    rejects(
        &format!("{t}fn wd(a: Acct, n: Int) -> Acct = a with {{ balance: a.balance - n }}"),
        Code::Refinement,
    );
    proves(&format!(
        "{t}fn wd(a: Acct, n: Int where it > 0) -> Acct = if n <= a.balance {{ a with {{ balance: a.balance - n }} }} else {{ a }}"
    ));
    // reading a field gives its refinement
    proves(&format!(
        "{t}fn f(a: Acct) -> Int ensures result >= 0 = a.balance"
    ));
    proves(&format!("{t}fn f(a: Acct) -> Int = 10 / (a.balance + 1)"));
}

#[test]
fn match_facts() {
    proves(
        "fn f(o: Option[Int]) -> Int ensures result >= 0 = match o {\n  Some(n) if n > 0 => n\n  Some(_) => 0\n  None => 0\n}",
    );
    rejects(
        "fn f(o: Option[Int]) -> Int ensures result > 0 = match o {\n  Some(n) => n\n  None => 1\n}",
        Code::Postcondition,
    );
    proves(
        "fn f(t: (Int, Int)) -> Int ensures result >= t.0 = match t {\n  (a, b) if b >= 0 => a + b\n  (a, _) => a\n}",
    );
    proves(
        "type S = A(Int) | B\nfn f(s: S) -> Int ensures result >= 0 = match s {\n  A(n) if n > 0 => n\n  A(_) => 0\n  B => 7\n}",
    );
}

#[test]
fn asserts() {
    proves("fn f(x: Int) -> Int requires x > 0 = {\n  assert x >= 1\n  x\n}");
    rejects(
        "fn f(x: Int) -> Int = {\n  assert x > 0\n  x\n}",
        Code::Assert,
    );
    // an assert is an assumption afterwards
    proves(
        "fn f(x: Int) -> Int requires x >= 0 = {\n  assert x != 3 || x > 2\n  assert x > 0 || x == 0\n  x\n}",
    );
}

#[test]
fn termination() {
    proves(
        "fn fact(n: Int) -> Int requires n >= 0 ensures result >= 1 decreases n = if n == 0 { 1 } else { n * fact(n - 1) }",
    );
    rejects(
        "fn f(n: Int) -> Int requires n >= 0 decreases n = if n == 0 { 0 } else { f(n) }",
        Code::Termination,
    );
    rejects(
        "fn f(n: Int) -> Int decreases n = if n == 0 { 0 } else { f(n - 1) }",
        Code::Termination,
    );
    rejects(
        "fn f(n: Int) -> Int requires n >= 0 decreases n = if n == 0 { 0 } else { f(n + 1) }",
        Code::Termination,
    );
    proves(
        "fn len(xs: List[Int]) -> Int decreases xs.len() = match xs {\n  [] => 0\n  [_, ..t] => 1 + len(t)\n}",
    );
    proves(
        "fn sum(xs: List[Int]) -> Int decreases xs.len() = if xs.len() == 0 { 0 } else { xs[0] + sum(xs.drop(1)) }",
    );
    rejects(
        "fn bad(xs: List[Int]) -> Int decreases xs.len() = match xs {\n  [] => 0\n  [h, ..t] => 1 + bad(xs)\n}",
        Code::Termination,
    );
    // mutual recursion
    proves(
        "fn ev(n: Int) -> Bool requires n >= 0 decreases n = if n == 0 { true } else { od(n - 1) }\nfn od(n: Int) -> Bool requires n >= 0 decreases n = if n == 0 { false } else { ev(n - 1) }",
    );
    rejects(
        "fn ev(n: Int) -> Bool requires n >= 0 decreases n = if n == 0 { true } else { od(n) }\nfn od(n: Int) -> Bool requires n >= 0 decreases n = if n == 0 { false } else { ev(n - 1) }",
        Code::Termination,
    );
}

#[test]
fn recursion_with_contracts() {
    proves(
        "fn sumTo(n: Int) -> Int requires n >= 0 ensures result >= 0 decreases n = if n == 0 { 0 } else { n + sumTo(n - 1) }",
    );
    rejects(
        "fn sumTo(n: Int) -> Int requires n >= 0 ensures result > 0 decreases n = if n == 0 { 0 } else { n + sumTo(n - 1) }",
        Code::Postcondition,
    );
    proves(
        "fn rev(xs: List[Int]) -> List[Int] ensures result.len() == xs.len() decreases xs.len() = match xs {\n  [] => []\n  [h, ..t] => rev(t).append(h)\n}",
    );
}

#[test]
fn floats() {
    proves("fn f(x: Float) -> Float = if x != 0.0 { 1.0 / x } else { 0.0 }");
    proves("fn f(x: Float) -> Float requires x > 0.0 = 1.0 / x");
    proves("fn f(x: Float) -> Float = x / 2.0");
    proves("fn f(x: Float) -> Float requires x >= 0.0 = x.sqrt()");
    proves("fn f(x: Float) -> Float = if x > 1.0 { (x - 1.0).sqrt() } else { 0.0 }");
    rejects("fn f(x: Float) -> Float = 1.0 / x", Code::FloatDomain);
    rejects("fn f(x: Float) -> Float = x.sqrt()", Code::FloatDomain);
    rejects(
        "fn f(x: Float) -> Float requires x >= 0.0 = 1.0 / x",
        Code::FloatDomain,
    );
}

#[test]
fn strings() {
    proves("fn f(s: Str) -> Str requires s.len() >= 3 = s.slice(1, 3)");
    rejects("fn f(s: Str) -> Str = s.slice(1, 3)", Code::Precondition);
    proves("fn f(a: Str, b: Str) -> Int ensures result == a.len() + b.len() = (a ++ b).len()");
    proves("fn f() -> Int ensures result == 5 = \"hello\".len()");
    proves("fn f(s: Str) -> List[Str] = s.split(\",\")");
    rejects(
        "fn f(s: Str, sep: Str) -> List[Str] = s.split(sep)",
        Code::Refinement,
    );
    proves("fn f(s: Str, sep: Str) -> List[Str] requires sep.len() > 0 = s.split(sep)");
}

#[test]
fn closures_and_functions_as_values() {
    proves("fn f(xs: List[Int]) -> List[Int] = xs.map(x => x * 2)");
    rejects(
        "fn f(xs: List[Int]) -> List[Int] = xs.map(x => 10 / x)",
        Code::DivZero,
    );
    proves("fn f(xs: List[Int]) -> List[Int] = xs.map(x => 10 / (x * x + 1))");
    // a function with a contract cannot be passed as a value
    rejects(
        "fn g(x: Int) -> Int requires x > 0 = x\nfn f(xs: List[Int]) -> List[Int] = xs.map(g)",
        Code::Precondition,
    );
    proves("fn g(x: Int) -> Int = x + 1\nfn f(xs: List[Int]) -> List[Int] = xs.map(g)");
    // recursive functions cannot hide recursion in a value
    // `g` is only applied to elements below `n`: terminates
    proves(
        "fn g(n: Int) -> Int requires n >= 0 decreases n = if n == 0 { 0 } else { List.range(0, 1).map(g).len() }",
    );
    // ... but an element may equal `n`
    rejects(
        "fn g(n: Int) -> Int requires n >= 0 decreases n = if n == 0 { 0 } else { List.range(0, n + 1).map(g).len() }",
        Code::Termination,
    );
    proves("fn f(o: Option[Int]) -> Int = o.map(x => x + 1).unwrapOr(0)");
    rejects(
        "fn f(o: Option[Int]) -> Int = o.map(x => 10 / x).unwrapOr(0)",
        Code::DivZero,
    );
    proves("fn f(x: Int) -> Int requires x != 0 = (y => 10 / y)(x)");
}

#[test]
fn counterexamples_are_reported() {
    need_z3!();
    let ps = run("fn f(a: Int, b: Int) -> Int requires a > 10 = a / b");
    let p = ps
        .iter()
        .find(|p| p.code == Code::DivZero)
        .expect("div zero");
    let ce = p.counterexample.as_ref().expect("counterexample");
    let get = |k: &str| ce.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    assert_eq!(get("b"), Some("0"));
    let a: i64 = get("a").unwrap().parse().unwrap();
    assert!(a > 10);
}

#[test]
fn unreachable_code_is_not_an_error() {
    proves("fn f(x: Int) -> Int requires x > 0 && x < 0 = 1 / 0");
}

#[test]
fn constants_are_verified() {
    proves("const a: Int = 10\nconst b: Int = a / 2\nfn f() -> Int ensures result == 5 = b");
    rejects("const a: Int = 1 / 0", Code::DivZero);
}

#[test]
fn type_level_refinements_must_be_well_defined() {
    rejects("type T = { n: Int where 10 / it > 1 }", Code::DivZero);
}

#[test]
fn generic_functions() {
    proves(
        "fn first[T](xs: List[T]) -> Option[T] = match xs {\n  [] => None\n  [h, ..t] => Some(h)\n}",
    );
    proves("fn f[T](xs: List[T]) -> Int ensures result == xs.len() = xs.len()");
    proves("fn swap[A, B](p: (A, B)) -> (B, A) = (p.1, p.0)");
    proves("fn f(xs: List[Int]) -> Int = {\n  let o = xs.first()\n  o.unwrapOr(0)\n}");
}

#[test]
fn structural_recursion_on_data() {
    let tree = "type Tree = Leaf | Node(Tree, Int, Tree)\n";
    proves(&format!(
        "{tree}fn size(t: Tree) -> Int ensures result >= 0 decreases t = match t {{\n  Leaf => 0\n  Node(l, _, r) => 1 + size(l) + size(r)\n}}"
    ));
    rejects(
        &format!(
            "{tree}fn size(t: Tree) -> Int decreases t = match t {{\n  Leaf => 0\n  Node(l, _, r) => 1 + size(t)\n}}"
        ),
        Code::Termination,
    );
    // a measure over one value used for another function (as in an evaluator)
    proves(
        "type E = Num(Int) | Add(E, E) | Neg(E)
fn size(e: E) -> Int ensures result >= 1 decreases e = match e {
  Num(_) => 1
  Add(a, b) => 1 + size(a) + size(b)
  Neg(a) => 1 + size(a)
}
fn eval(e: E) -> Int decreases e = match e {
  Num(n) => n
  Add(a, b) => eval(a) + eval(b)
  Neg(a) => 0 - eval(a)
}",
    );
}

#[test]
fn diverging_functions_are_rejected_not_trusted() {
    // the defining equation `f(x) = f(x) + 1` is contradictory; it must never be assumed
    rejects(
        "fn f(x: Int) -> Int decreases x = f(x) + 1",
        Code::Termination,
    );
    rejects(
        "fn f(x: Int) -> Int requires x > 0 decreases x = if x > 0 { f(x) + 1 } else { 0 }\nfn g() -> Int ensures result == 1 = 1 / 0",
        Code::Termination,
    );
}

#[test]
fn recursive_definitions_can_be_unfolded() {
    proves(
        "fn sumTo(n: Int) -> Int requires n >= 0 decreases n = if n == 0 { 0 } else { n + sumTo(n - 1) }\nfn f() -> Int ensures result == 10 = sumTo(4)",
    );
    // induction is done by contract: the postcondition also holds for the recursive call
    proves(
        "fn len(xs: List[Int]) -> Int ensures result == xs.len() decreases xs.len() = match xs {\n  [] => 0\n  [_, ..t] => 1 + len(t)\n}",
    );
}

#[test]
fn elements_of_lists_are_known_inside_lambdas() {
    proves("fn f(n: Int) -> Bool = n >= 2 && List.range(2, n).all(d => n % d != 0)");
    proves("fn f(xs: List[Int]) -> List[Int] requires xs.all(x => x > 0) = xs");
    proves(
        "fn g(x: Int) -> Int requires x >= 0 = x\nfn f() -> List[Int] = List.range(0, 5).map(g)",
    );
    rejects(
        "fn g(x: Int) -> Int requires x > 0 = x\nfn f() -> List[Int] = List.range(0, 5).map(g)",
        Code::Precondition,
    );
}

fn run_cached(src: &str, dir: &std::path::Path) -> Vec<Problem> {
    let parsed = parse(src, ParseOptions::default());
    let out = check(&parsed.file, src);
    assert!(out.problems.is_empty(), "check: {:#?}", out.problems);
    verify(
        &out.program,
        src,
        &VerifyOptions {
            timeout_ms: 6000,
            include_prelude: false,
            cache_dir: Some(dir.to_path_buf()),
            backend: tez_verify::Backend::Process,
        },
    )
    .unwrap()
}

#[test]
fn proofs_are_cached_and_refutations_never_are() {
    need_z3!();
    let dir = std::env::temp_dir().join(format!("tez-vc-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let good = "fn f(xs: List[Int], i: Int) -> Int\n  requires 0 <= i && i < xs.len()\n  = xs.get(i)\n\nfn main() -> Int = f([1, 2, 3], 1)\n";
    let bad = "fn f(xs: List[Int], i: Int) -> Int = xs.get(i)\n";

    let before = tez_verify::CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed);
    assert!(run_cached(good, &dir).is_empty());
    assert_eq!(
        tez_verify::CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed),
        before,
        "first run solves everything"
    );
    assert!(
        std::fs::read_dir(&dir).unwrap().count() > 0,
        "proofs are stored"
    );

    assert!(run_cached(good, &dir).is_empty());
    assert!(
        tez_verify::CACHE_HITS.load(std::sync::atomic::Ordering::Relaxed) > before,
        "second run is answered from the cache"
    );

    // a program that can fail is rejected every time, with the cache warm or cold
    for _ in 0..2 {
        let ps = run_cached(bad, &dir);
        assert!(
            ps.iter().any(|p| p.code == tez_diag::Code::IndexBounds),
            "{ps:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Programs found by review (2026-10-15) that used to verify and then failed at run time.
#[test]
fn review_regressions_are_rejected() {
    need_z3!();
    // Str.upper/lower used to promise `result.len() == self.len()`, false for "ß" and "İ"
    for f in ["upper", "lower"] {
        let src = format!(
            "fn f(s: Str) -> Int\n  requires s.{f}().len() == 2\n  = 10 / (s.len() - 1)\n\nfn main() -> Int = {{\n  let c = \"aß\".chars().get(1)\n  if c.{f}().len() == 2 {{ f(c) }} else {{ 0 }}\n}}\n"
        );
        let ps = run(&src);
        assert!(ps.iter().any(|p| p.code == Code::DivZero), "{f}: {ps:?}");
    }
}

#[test]
fn structural_measures_are_inferred_and_proved() {
    need_z3!();
    let ok = [
        "fn sum(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [h, ..t] => h + sum(t)\n}\n",
        "type T = Leaf | Node(T, Int, T)\n\nfn size(t: T) -> Int = match t {\n  Leaf => 0\n  Node(l, _, r) => size(l) + 1 + size(r)\n}\n",
        "fn pairs(xs: List[Int]) -> Int = match xs {\n  [a, b, ..rest] => a * b + pairs(rest)\n  _ => 0\n}\n",
    ];
    for src in ok {
        assert!(run(src).is_empty(), "{src}");
    }
}
