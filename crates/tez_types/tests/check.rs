//! Type checker tests: every accepted shape and every error code it can produce.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use tez_diag::Code;
use tez_syntax::{ParseOptions, parse};
use tez_types::{check, hir};

fn run(src: &str) -> (hir::Program, Vec<(Code, String)>) {
    let parsed = parse(src, ParseOptions::default());
    assert!(
        parsed.problems.is_empty(),
        "parse problems: {:?}",
        parsed.problems
    );
    let out = check(&parsed.file, src);
    let probs = out
        .problems
        .iter()
        .map(|p| (p.code, p.message.clone()))
        .collect();
    (out.program, probs)
}

fn ok(src: &str) -> hir::Program {
    let (p, probs) = run(src);
    assert!(
        probs.is_empty(),
        "unexpected problems: {probs:#?}\nsource:\n{src}"
    );
    p
}

fn codes(src: &str) -> Vec<Code> {
    run(src).1.into_iter().map(|(c, _)| c).collect()
}

fn has(src: &str, code: Code) {
    let (_, probs) = run(src);
    assert!(
        probs.iter().any(|(c, _)| *c == code),
        "expected {code:?}, got {probs:#?}\nsource:\n{src}"
    );
}

#[test]
fn prelude_checks_cleanly() {
    let p = ok("");
    assert!(p.types.iter().any(|t| t.name == "Option"));
    assert!(p.find_fn("List.map").is_some());
    assert!(p.find_fn("Int.abs").is_some());
}

#[test]
fn simple_programs_check() {
    ok("fn add(a: Int, b: Int) -> Int = a + b");
    ok("fn half(x: Float) -> Float = x / 2.0");
    ok("fn greet(name: Str) -> Str = \"hi \" ++ name");
    ok("fn f(xs: List[Int]) -> List[Int] = xs.map(x => x * 2).filter(x => x > 2)");
    ok("fn total(xs: List[Int]) -> Int = xs.fold(0, (acc, x) => acc + x)");
    ok("fn f(o: Option[Int]) -> Int = match o {\n  Some(n) => n\n  None => 0\n}");
    ok("fn f() -> List[Int] = List.range(0, 10)");
    ok("fn f(s: Str) -> Option[Int] = s.trim().toInt()");
    ok("fn f(n: Int) -> Float = n.toFloat() / 2.0");
}

#[test]
fn types_records_and_adts() {
    ok("type Point = { x: Int, y: Int }
fn moveRight(p: Point) -> Point = p with { x: p.x + 1 }
fn origin() -> Point = Point { x: 0, y: 0 }
fn getX(p: Point) -> Int = p.x");
    ok("type Shape = Circle(Int) | Rect(Int, Int)
fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Rect(w, h) => w * h
}");
    ok("type Tree[T] = Leaf | Node(Tree[T], T, Tree[T])
fn size[T](t: Tree[T]) -> Int
  decreases 0
  = match t {
  Leaf => 0
  Node(l, _, r) => 1 + size(l) + size(r)
}");
    ok("type Pair[A, B] = { first: A, second: B }
fn swap[A, B](p: Pair[A, B]) -> Pair[B, A] = Pair { first: p.second, second: p.first }");
    ok("type Id = Int\nfn f(x: Id) -> Int = x");
    ok("type Wrapper[T] = List[T]\nfn f(x: Wrapper[Int]) -> Int = x.len()");
}

#[test]
fn lambdas_infer_from_context() {
    ok("fn f(xs: List[Int]) -> List[Str] = xs.map(x => x.toStr())");
    ok("fn f(xs: List[(Int, Str)]) -> List[Int] = xs.map(p => p.0)");
    ok("fn f(xs: List[Int]) -> Int = xs.fold(0, (a, b) => a + b)");
    ok("fn apply(f: (Int) -> Int, x: Int) -> Int = f(x)\nfn g() -> Int = apply(x => x + 1, 2)");
    ok("fn add1(x: Int) -> Int = x + 1\nfn g(xs: List[Int]) -> List[Int] = xs.map(add1)");
}

#[test]
fn let_blocks_and_tuples() {
    ok("fn f(x: Int) -> Int = {
  let y = x + 1
  let (a, b) = (y, y * 2)
  assert a < b || a == b
  a + b
}");
    ok("fn f(x: Int) -> Int = {\n  let y: Int = x\n  y\n}");
}

#[test]
fn constants_and_tests() {
    ok(
        "const limit: Int = 100\nconst double: Int = limit * 2\nfn f() -> Int = limit + double\ntest t = f() == 300",
    );
}

#[test]
fn contracts_check() {
    let p = ok("fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  ensures lo <= result && result <= hi
  = if x < lo { lo } else if x > hi { hi } else { x }");
    let f = &p.fns[p.find_fn("clamp").unwrap()];
    assert_eq!(f.requires.len(), 1);
    assert_eq!(f.ensures.len(), 1);
    ok("fn d(a: Int, b: Int where it != 0) -> Int = a / b");
    ok("type Pos = Int where it > 0\nfn f(x: Pos) -> Int = x");
    ok("type NonEmpty[T] = List[T] where it.len() > 0\nfn f(xs: NonEmpty[Int]) -> Int = xs[0]");
    ok("fn f(lo: Int, hi: Int where it >= lo) -> Int = hi - lo");
}

#[test]
fn recursion_needs_decreases() {
    ok("fn fact(n: Int) -> Int
  requires n >= 0
  decreases n
  = if n == 0 { 1 } else { n * fact(n - 1) }");
    // countdown by a literal is inferred (the verifier still has to prove it) ...
    ok("fn fact(n: Int) -> Int = if n == 0 { 1 } else { n * fact(n - 1) }");
    // ... counting up is not
    has(
        "fn up(n: Int) -> Int = if n >= 100 { n } else { up(n + 1) }",
        Code::TerminationMissing,
    );
    has(
        "fn isEven(n: Int) -> Bool = if n == 0 { true } else { isOdd(n - 1) }
fn isOdd(n: Int) -> Bool = if n == 0 { false } else { isEven(n - 1) }",
        Code::TerminationMissing,
    );
    ok("fn f(n: Int) -> Int = n\nfn g(n: Int) -> Int = f(n)");
}

// ------------------------------------------------------------------ names

#[test]
fn unknown_names() {
    has("fn f() -> Int = nope", Code::NameUnknown);
    has("fn f() -> Foo = 1", Code::NameUnknownType);
    has("fn f() -> Int = Nope(1)", Code::NameUnknownCtor);
    has("fn f(x: Int) -> Int = x.nope()", Code::NameUnknownMethod);
    has("fn f() -> Int = List.nope(1)", Code::NameUnknownMethod);
    has(
        "fn f(o: Option[Int]) -> Int = match o {\n  Nope => 1\n  _ => 2\n}",
        Code::NameUnknownCtor,
    );
}

#[test]
fn suggestions_for_typos() {
    let src = "fn f(count: Int) -> Int = cunt";
    let parsed = parse(src, ParseOptions::default());
    let out = check(&parsed.file, src);
    let p = out
        .problems
        .iter()
        .find(|p| p.code == Code::NameUnknown)
        .unwrap();
    assert_eq!(p.fixes[0].replace, "count");
}

#[test]
fn duplicates_shadowing_and_prelude() {
    has("fn f() -> Int = 1\nfn f() -> Int = 2", Code::NameDuplicate);
    has("type A = Int\ntype A = Str", Code::NameDuplicate);
    has("type S = X | X", Code::NameDuplicate);
    has("type P = { x: Int, x: Int }", Code::NameDuplicate);
    has("fn f(a: Int, a: Int) -> Int = a", Code::NameDuplicate);
    has("type A = X(Int)\ntype B = X(Int)", Code::NameDuplicate);
    has(
        "fn f(x: Int) -> Int = {\n  let x = 1\n  x\n}",
        Code::NameShadowing,
    );
    has(
        "fn g() -> Int = 1\nfn f(g: Int) -> Int = g",
        Code::NameShadowing,
    );
    has(
        "fn f(xs: List[Int]) -> List[Int] = xs.map(xs => xs)",
        Code::NameShadowing,
    );
    has("type Option[T] = A | B", Code::NamePrelude);
    has("type Int = Str", Code::NamePrelude);
    has("type MyOpt = Some(Int)", Code::NamePrelude);
}

#[test]
fn method_syntax_is_enforced_with_a_fix() {
    let src = "fn f(xs: List[Int]) -> Int = List.len(xs)";
    let parsed = parse(src, ParseOptions::default());
    let out = check(&parsed.file, src);
    let p = out
        .problems
        .iter()
        .find(|p| p.code == Code::NameUseMethodSyntax)
        .unwrap();
    assert_eq!(p.fixes[0].replace, "xs.len()");
    let src = "fn f(xs: List[Int], g: (Int) -> Int) -> List[Int] = List.map(xs, g)";
    let parsed = parse(src, ParseOptions::default());
    let out = check(&parsed.file, src);
    let p = out
        .problems
        .iter()
        .find(|p| p.code == Code::NameUseMethodSyntax)
        .unwrap();
    assert_eq!(p.fixes[0].replace, "xs.map(g)");
}

#[test]
fn contract_scope() {
    has(
        "fn f(x: Int) -> Int requires result > 0 = x",
        Code::ContractScope,
    );
    has("fn f(x: Int) -> Int = it", Code::ContractScope);
}

#[test]
fn const_initializers_are_restricted() {
    has("fn f() -> Int = 1\nconst c: Int = f()", Code::ConstInit);
    has("const a: Int = b\nconst b: Int = 1", Code::ConstInit);
    ok("const a: Int = [1, 2].len()");
}

// ------------------------------------------------------------------ types

#[test]
fn mismatches() {
    has("fn f() -> Int = \"x\"", Code::TypeMismatch);
    has("fn f(x: Int) -> Float = x", Code::TypeMismatch);
    has(
        "fn f(x: Int) -> Bool = if x { true } else { false }",
        Code::TypeMismatch,
    );
    has(
        "fn f() -> Int = if true { 1 } else { \"a\" }",
        Code::TypeMismatch,
    );
    has("fn f() -> Int = 1 + 2.0", Code::TypeMismatch);
    has("fn f(xs: List[Int]) -> List[Str] = xs", Code::TypeMismatch);
    has("fn f() -> List[Int] = [1, \"a\"]", Code::TypeMismatch);
    has(
        "fn g(x: Int) -> Int = x\nfn f() -> Int = g(\"a\")",
        Code::TypeMismatch,
    );
    has(
        "type P = { x: Int }\nfn f() -> P = P { x: \"a\" }",
        Code::TypeMismatch,
    );
}

#[test]
fn arity_errors() {
    has(
        "fn g(x: Int) -> Int = x\nfn f() -> Int = g(1, 2)",
        Code::TypeArity,
    );
    has("fn f(xs: List[Int]) -> Int = xs.len(1)", Code::TypeArity);
    has("fn f() -> Option[Int] = Some(1, 2)", Code::TypeArity);
    has("fn f() -> Option[Int] = Some", Code::TypeArity);
    has("fn f(x: List) -> Int = 1", Code::TypeArity);
    has(
        "fn f(o: Option[Int]) -> Int = match o {\n  Some(a, b) => a\n  None => 0\n}",
        Code::TypeArity,
    );
    has(
        "fn f(xs: List[Int]) -> List[Int] = xs.map((a, b) => a)",
        Code::TypeArity,
    );
    has("fn f(g: (Int) -> Int) -> Int = g(1, 2)", Code::TypeArity);
}

#[test]
fn not_a_function() {
    has("fn f(x: Int) -> Int = x(1)", Code::TypeNotFunction);
    has(
        "const c: Int = 1\nfn f() -> Int = c(1)",
        Code::TypeNotFunction,
    );
}

#[test]
fn record_errors() {
    let p = "type P = { x: Int, y: Int }\n";
    has(
        &format!("{p}fn f(q: P) -> Int = q.z"),
        Code::TypeUnknownField,
    );
    has(
        &format!("{p}fn f() -> P = P {{ x: 1 }}"),
        Code::TypeMissingField,
    );
    has(
        &format!("{p}fn f() -> P = P {{ x: 1, y: 2, z: 3 }}"),
        Code::TypeExtraField,
    );
    has(
        &format!("{p}fn f() -> P = P {{ x: 1, x: 2, y: 3 }}"),
        Code::TypeExtraField,
    );
    has(
        &format!("{p}fn f(q: P) -> P = q with {{ z: 1 }}"),
        Code::TypeUnknownField,
    );
    has("fn f(x: Int) -> Int = x.y", Code::TypeUnknownField);
    has(
        "type S = A | B\nfn f(s: S) -> Int = s.x",
        Code::TypeUnknownField,
    );
    has("type P = { x: Int }\nfn f() -> P = P", Code::TypeArity);
}

#[test]
fn ambiguity() {
    has(
        "fn f() -> Int = {\n  let xs = []\n  1\n}",
        Code::TypeAmbiguous,
    );
    has("fn f() -> Int = [].len()", Code::TypeAmbiguous);
    has(
        "fn f() -> Int = {\n  let g = x => x.len()\n  1\n}",
        Code::TypeAmbiguous,
    );
    ok("fn f() -> Int = {\n  let xs: List[Int] = []\n  xs.len()\n}");
}

#[test]
fn infinite_types() {
    has(
        "fn f() -> Int = {\n  let g = x => x(x)\n  1\n}",
        Code::TypeInfinite,
    );
}

#[test]
fn bool_conditions() {
    has(
        "fn f(x: Int) -> Int = if x { 1 } else { 2 }",
        Code::TypeMismatch,
    );
    has("fn f(x: Int) -> Int requires x = x", Code::TypeMismatch);
    has(
        "fn f(x: Int) -> Int = {\n  assert x\n  x\n}",
        Code::TypeMismatch,
    );
    has(
        "fn f(x: Int) -> Int = match x {\n  n if n => 1\n  _ => 2\n}",
        Code::TypeMismatch,
    );
    has("test t = 1", Code::TypeMismatch);
}

#[test]
fn operator_operands() {
    has("fn f(a: Str, b: Str) -> Str = a + b", Code::TypeBadOperand);
    has("fn f(a: Bool) -> Bool = -a", Code::TypeBadOperand);
    has(
        "fn f(a: Float, b: Float) -> Float = a % b",
        Code::TypeBadOperand,
    );
    has("fn f(a: Int, b: Str) -> Str = a ++ b", Code::TypeMismatch);
    has("fn f(a: Int) -> Int = a[0]", Code::TypeBadOperand);
    has("fn f(a: Int) -> Int = a.0", Code::TypeTupleIndex);
}

#[test]
fn equality_and_ordering() {
    ok("fn f(a: Str, b: Str) -> Bool = a == b && a < b");
    ok("fn f(a: List[Int], b: List[Int]) -> Bool = a == b");
    ok("type P = { x: Int }\nfn f(a: P, b: P) -> Bool = a == b");
    has(
        "fn f(a: (Int) -> Int, b: (Int) -> Int) -> Bool = a == b",
        Code::TypeNotEquatable,
    );
    has(
        "type W = { f: (Int) -> Int }\nfn g(a: W, b: W) -> Bool = a == b",
        Code::TypeNotEquatable,
    );
    has(
        "fn f(a: Bool, b: Bool) -> Bool = a < b",
        Code::TypeNotOrdered,
    );
    has(
        "fn f(a: List[Int], b: List[Int]) -> Bool = a < b",
        Code::TypeNotOrdered,
    );
}

#[test]
fn tuple_index() {
    ok("fn f(t: (Int, Str)) -> Str = t.1");
    has("fn f(t: (Int, Str)) -> Str = t.2", Code::TypeTupleIndex);
}

#[test]
fn refinement_position_and_alias_cycles() {
    has(
        "fn f(xs: List[Int where it > 0]) -> Int = 1",
        Code::TypeRefinementPosition,
    );
    has(
        "type Pos = Int where it > 0\nfn f(xs: List[Pos]) -> Int = 1",
        Code::TypeRefinementPosition,
    );
    has(
        "fn f(g: (Int where it > 0) -> Int) -> Int = 1",
        Code::TypeRefinementPosition,
    );
    has("type A = B\ntype B = A", Code::TypeRecursiveAlias);
}

// ------------------------------------------------------------------ patterns

#[test]
fn exhaustiveness() {
    ok("fn f(o: Option[Int]) -> Int = match o {\n  Some(n) => n\n  None => 0\n}");
    has(
        "fn f(o: Option[Int]) -> Int = match o {\n  Some(n) => n\n}",
        Code::MatchNonExhaustive,
    );
    has(
        "fn f(b: Bool) -> Int = match b {\n  true => 1\n}",
        Code::MatchNonExhaustive,
    );
    has(
        "fn f(n: Int) -> Int = match n {\n  0 => 1\n  1 => 2\n}",
        Code::MatchNonExhaustive,
    );
    has(
        "fn f(s: Str) -> Int = match s {\n  \"a\" => 1\n}",
        Code::MatchNonExhaustive,
    );
    ok("fn f(n: Int) -> Int = match n {\n  0 => 1\n  _ => 2\n}");
    ok("fn f(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [h, ..t] => h\n}");
    has(
        "fn f(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [a] => a\n}",
        Code::MatchNonExhaustive,
    );
    ok(
        "fn f(t: (Bool, Bool)) -> Int = match t {\n  (true, _) => 1\n  (false, true) => 2\n  (false, false) => 3\n}",
    );
    has(
        "fn f(t: (Bool, Bool)) -> Int = match t {\n  (true, _) => 1\n  (false, true) => 2\n}",
        Code::MatchNonExhaustive,
    );
    has(
        "fn f(o: Option[Int]) -> Int = match o {\n  Some(n) if n > 0 => n\n  None => 0\n}",
        Code::MatchNonExhaustive,
    );
    ok(
        "fn f(o: Option[Option[Int]]) -> Int = match o {\n  Some(Some(n)) => n\n  Some(None) => 0\n  None => 0\n}",
    );
    has(
        "fn f(o: Option[Option[Int]]) -> Int = match o {\n  Some(Some(n)) => n\n  None => 0\n}",
        Code::MatchNonExhaustive,
    );
}

#[test]
fn exhaustiveness_witness_is_reported() {
    let src = "type S = A | B(Int) | C\nfn f(s: S) -> Int = match s {\n  A => 1\n  C => 2\n}";
    let parsed = parse(src, ParseOptions::default());
    let out = check(&parsed.file, src);
    let p = out
        .problems
        .iter()
        .find(|p| p.code == Code::MatchNonExhaustive)
        .unwrap();
    assert!(p.message.contains("B(_)"), "{}", p.message);
}

#[test]
fn unreachable_arms() {
    has(
        "fn f(o: Option[Int]) -> Int = match o {\n  _ => 0\n  Some(n) => n\n}",
        Code::MatchUnreachable,
    );
    has(
        "fn f(b: Bool) -> Int = match b {\n  true => 1\n  false => 2\n  true => 3\n}",
        Code::MatchUnreachable,
    );
    ok("fn f(n: Int) -> Int = match n {\n  x if x > 0 => 1\n  _ => 0\n}");
}

#[test]
fn refutable_let_patterns() {
    has(
        "fn f(o: Option[Int]) -> Int = {\n  let Some(x) = o\n  x\n}",
        Code::MatchNonExhaustive,
    );
    ok("fn f(t: (Int, Int)) -> Int = {\n  let (a, b) = t\n  a + b\n}");
}

#[test]
fn pattern_bindings() {
    has(
        "fn f(t: (Int, Int)) -> Int = match t {\n  (a, a) => a\n}",
        Code::MatchBindingMismatch,
    );
    has(
        "fn f(n: Int) -> Int = match n {\n  \"a\" => 1\n  _ => 2\n}",
        Code::TypeMismatch,
    );
    has(
        "fn f(o: Option[Int]) -> Int = match o {\n  Ok(n) => n\n  _ => 0\n}",
        Code::TypeMismatch,
    );
}

#[test]
fn errors_are_reported_together() {
    let cs = codes("fn a() -> Int = \"x\"\nfn b() -> Int = nope\nfn c() -> Str = 1");
    assert_eq!(cs.len(), 3, "{cs:?}");
}

#[test]
fn hir_types_are_fully_resolved() {
    let p = ok("fn f(xs: List[Int]) -> List[Int] = xs.map(x => x + 1)");
    let f = &p.fns[p.find_fn("f").unwrap()];
    let body = f.body.as_ref().unwrap();
    assert!(!body.ty.has_var());
    body.walk(&mut |e| {
        assert!(
            !e.ty.has_var() && !e.ty.has_error(),
            "unresolved type at {:?}",
            e.span
        )
    });
}

#[test]
fn float_literals_must_be_finite() {
    let big = format!("1{}.0", "0".repeat(400));
    let src = format!("fn f() -> Float = {big}\n");
    let parsed = tez_syntax::parse(&src, tez_syntax::ParseOptions::default());
    let out = tez_types::check(&parsed.file, &src);
    assert!(
        out.problems
            .iter()
            .any(|p| p.code == tez_diag::Code::LexBadNumber),
        "{:?}",
        out.problems
    );
}

#[test]
fn calling_a_function_field_suggests_parentheses() {
    let src = "type Box = { f: (Int) -> Int }\n\nfn use(b: Box) -> Int = b.f(0)\n";
    let parsed = tez_syntax::parse(src, tez_syntax::ParseOptions::default());
    let out = tez_types::check(&parsed.file, src);
    let p = out.problems.first().expect("a problem");
    assert!(p.message.contains("(b.f)(..)"), "{}", p.message);
}

#[test]
fn non_structural_recursion_still_needs_decreases() {
    for src in [
        "fn bad(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [h, ..t] => bad(xs)\n}\n",
        "fn bad(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [h, ..t] => bad(t.append(h))\n}\n",
        "fn up(n: Int) -> Int = if n >= 100 { n } else { up(n + 1) }\n",
        "fn halve(n: Int) -> Int = if n <= 1 { n } else { halve(n / 2) }\n",
    ] {
        let parsed = tez_syntax::parse(src, tez_syntax::ParseOptions::default());
        let out = tez_types::check(&parsed.file, src);
        assert!(
            out.problems
                .iter()
                .any(|p| p.code == tez_diag::Code::TerminationMissing),
            "{src}"
        );
    }
}
