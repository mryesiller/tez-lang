use tez_diag::Code;

use super::{ParseOptions, parse};
use crate::ast::{BinOp, ExprKind, File, Item, PatKind, TypeBody};

fn ok(src: &str) -> File {
    let out = parse(src, ParseOptions::default());
    assert!(
        out.problems.is_empty(),
        "unexpected problems: {:#?}\nsource:\n{src}",
        out.problems
    );
    out.file
}

fn codes(src: &str) -> Vec<Code> {
    parse(src, ParseOptions::default())
        .problems
        .iter()
        .map(|p| p.code)
        .collect()
}

fn body_of(src: &str) -> crate::ast::Expr {
    match ok(src).items.into_iter().next() {
        Some(Item::Fn(f)) => f.body.expect("body"),
        other => panic!("expected fn, got {other:?}"),
    }
}

#[test]
fn function_with_contracts_on_following_lines() {
    let f = ok("fn avg(xs: List[Int]) -> Int
  requires xs.len() > 0
  ensures result >= 0
  = xs.sum() / xs.len()
");
    let Item::Fn(f) = &f.items[0] else { panic!() };
    assert_eq!(f.requires.len(), 1);
    assert_eq!(f.ensures.len(), 1);
    assert!(f.body.is_some());
}

#[test]
fn generics_and_function_types() {
    ok("fn map2[A, B](xs: List[A], f: (A) -> B) -> List[B] = xs.map(f)");
    ok("fn thunk(f: () -> Int) -> Int = f()");
    ok("fn pair(p: (Int, Str)) -> (Str, Int) = (p.1, p.0)");
    ok("fn twice(f: (Int) -> Int) -> (Int) -> Int = x => f(f(x))");
}

#[test]
fn refinement_types() {
    ok("type Pos = Int where it > 0");
    ok("fn div(a: Int, b: Int where it != 0) -> Int where it >= 0 - 1000 = a / b");
}

#[test]
fn type_declarations() {
    let f = ok("type Point = { x: Int, y: Int }
type Shape = Circle(Int) | Rect(Int, Int)
type Dir =
  | Up
  | Down
type Tree[T] = Leaf | Node(Tree[T], T, Tree[T])
type Name = Str
type Wrapped = Boxed(Int)
type Ids = List[Int]
");
    let bodies: Vec<&TypeBody> = f
        .items
        .iter()
        .map(|i| match i {
            Item::Type(t) => &t.body,
            _ => panic!(),
        })
        .collect();
    assert!(matches!(bodies[0], TypeBody::Record(fs) if fs.len() == 2));
    assert!(matches!(bodies[1], TypeBody::Adt(vs) if vs.len() == 2));
    assert!(matches!(bodies[2], TypeBody::Adt(vs) if vs.len() == 2));
    assert!(matches!(bodies[3], TypeBody::Adt(vs) if vs.len() == 2));
    assert!(matches!(bodies[4], TypeBody::Alias(_)));
    assert!(matches!(bodies[5], TypeBody::Adt(vs) if vs.len() == 1));
    assert!(matches!(bodies[6], TypeBody::Alias(_)));
}

#[test]
fn const_and_test_items() {
    ok("const limit: Int = 100\ntest ok1 = limit == 100");
}

#[test]
fn precedence() {
    let e = body_of("fn f() -> Int = 1 + 2 * 3 - 4");
    // (1 + (2*3)) - 4
    let ExprKind::Binary(BinOp::Sub, l, _) = e.kind else {
        panic!("{e:?}")
    };
    assert!(matches!(l.kind, ExprKind::Binary(BinOp::Add, ..)));
    let e = body_of("fn f() -> Bool = a == 1 && b != 2");
    assert!(matches!(e.kind, ExprKind::Binary(BinOp::And, ..)));
    let e = body_of("fn f() -> Str = a ++ b ++ c");
    assert!(matches!(e.kind, ExprKind::Binary(BinOp::Concat, ..)));
}

#[test]
fn unary_and_postfix() {
    let e = body_of("fn f() -> Int = -x.abs()");
    assert!(matches!(e.kind, ExprKind::Unary(crate::ast::UnOp::Neg, _)));
    ok("fn f(t: (Int, (Int, Int))) -> Int = t.1.0");
    ok("fn f(xs: List[Int]) -> Int = xs[0] + xs[1]");
}

#[test]
fn method_chains_across_lines() {
    ok("fn f(xs: List[Int]) -> List[Int] =
  xs
    .filter(x => x > 0)
    .map(x => x * 2)
");
}

#[test]
fn logical_operators_continue_on_next_line() {
    ok("fn f(a: Bool, b: Bool) -> Bool =
  a
    && b
");
}

#[test]
fn lambdas() {
    ok("fn f() -> Int = g(x => x + 1)");
    ok("fn f() -> Int = g((a, b) => a + b)");
    ok("fn f() -> Int = g((a: Int, b: Int) => a + b)");
    ok("fn f() -> Int = g(() => 1)");
    ok("fn f() -> Int = (x => x)(3)");
}

#[test]
fn if_match_and_blocks() {
    ok("fn f(n: Int) -> Int = if n > 0 { 1 } else if n < 0 { -1 } else { 0 }");
    let f = ok("fn f(o: Option[Int]) -> Int = match o {
  Some(n) if n > 0 => n
  Some(_) => 0
  None => -1
}
");
    let Item::Fn(fd) = &f.items[0] else { panic!() };
    let ExprKind::Match { arms, .. } = &fd.body.as_ref().unwrap().kind else {
        panic!()
    };
    assert_eq!(arms.len(), 3);
    assert!(arms[0].guard.is_some());
    ok("fn f(x: Int) -> Int = {
  let y = x + 1
  let (a, b) = (y, y)
  assert a == b
  a
}");
}

#[test]
fn records_with_and_paths() {
    ok("fn f(p: Point) -> Point = Point { x: 1, y: p.y }");
    ok("fn f(p: Point) -> Point = p with { x: p.x + 1 }");
    ok("fn f() -> List[Int] = List.range(0, 10)");
    ok("fn f(p: Point) -> Int = if p.x > 0 { 1 } else { 2 }");
}

#[test]
fn record_literal_not_allowed_in_conditions() {
    // `Red {` here is a constructor followed by a block, not a record literal
    ok("fn f(c: Color) -> Int = if c == Red { 1 } else { 2 }");
    ok("fn f(c: Color) -> Int = match c {\n  Red => 1\n  _ => 2\n}");
}

#[test]
fn patterns() {
    ok("fn f(xs: List[Int]) -> Int = match xs {
  [] => 0
  [a] => a
  [a, b] => a + b
  [h, ..t] => h
}");
    ok("fn f(x: Int) -> Int = match x {
  0 => 1
  -1 => 2
  _ => 3
}");
    ok("fn f(t: (Int, Bool)) -> Int = match t {\n  (0, true) => 1\n  (_, _) => 2\n}");
    let f = ok("fn f(xs: List[Int]) -> Int = match xs {\n  [_, ..] => 1\n  [] => 0\n}");
    let Item::Fn(fd) = &f.items[0] else { panic!() };
    let ExprKind::Match { arms, .. } = &fd.body.as_ref().unwrap().kind else {
        panic!()
    };
    assert!(matches!(
        &arms[0].pat.kind,
        PatKind::List {
            rest: Some(None),
            ..
        }
    ));
}

#[test]
fn comments_and_trailing_commas() {
    ok("// header\nfn f(a: Int, b: Int,) -> Int = g(a, b,) // trailing\n");
    ok("fn f() -> List[Int] = [\n  1,\n  2,\n]");
}

#[test]
fn prelude_mode_allows_extern_and_qualified_names() {
    let out = parse(
        "extern fn List.get[A](self: List[A], i: Int) -> A requires 0 <= i && i < self.len()\n",
        ParseOptions { prelude: true },
    );
    assert!(out.problems.is_empty(), "{:?}", out.problems);
    let Item::Fn(f) = &out.file.items[0] else {
        panic!()
    };
    assert!(f.is_extern && f.body.is_none());
    assert_eq!(f.module.as_ref().unwrap().name, "List");
}

// ------------------------------------------------------------------ errors

#[test]
fn mixed_logic_is_reported() {
    assert!(
        codes("fn f(a: Bool, b: Bool, c: Bool) -> Bool = a && b || c")
            .contains(&Code::ParseMixedLogic)
    );
    assert!(
        codes("fn f(a: Bool, b: Bool, c: Bool) -> Bool = a || b && c")
            .contains(&Code::ParseMixedLogic)
    );
    ok("fn f(a: Bool, b: Bool, c: Bool) -> Bool = (a && b) || c");
}

#[test]
fn chained_comparison_is_reported() {
    assert!(codes("fn f(a: Int) -> Bool = 0 < a < 10").contains(&Code::ParseChainedComparison));
}

#[test]
fn if_without_else_is_reported() {
    assert!(codes("fn f(a: Bool) -> Int = if a { 1 }").contains(&Code::ParseElseRequired));
}

#[test]
fn block_without_tail_is_reported() {
    assert!(codes("fn f() -> Int = {\n  let x = 1\n}").contains(&Code::ParseBlockNoTail));
}

#[test]
fn stray_separators() {
    assert!(
        codes("fn f(x: Int) -> Int = match x {\n  0 => 1,\n  _ => 2\n}")
            .contains(&Code::ParseStraySeparator)
    );
    assert!(codes("fn f() -> Int = {\n  let x = 1;\n  x\n}").contains(&Code::ParseStraySeparator));
}

#[test]
fn naming_conventions() {
    let out = parse(
        "fn my_func(max_size: Int) -> Int = max_size",
        ParseOptions::default(),
    );
    let case: Vec<_> = out
        .problems
        .iter()
        .filter(|p| p.code == Code::ParseNameCase)
        .collect();
    assert_eq!(case.len(), 2);
    assert_eq!(case[0].fixes[0].replace, "myFunc");
    assert_eq!(case[1].fixes[0].replace, "maxSize");
    assert!(codes("type point = { x: Int }").contains(&Code::ParseNameCase));
    assert!(codes("const MAX: Int = 1").contains(&Code::ParseNameCase));
}

#[test]
fn reserved_words() {
    assert!(codes("fn match() -> Int = 1").contains(&Code::ParseReservedWord));
    assert!(codes("extern fn f() -> Int").contains(&Code::ParseReservedWord));
}

#[test]
fn duplicate_decreases() {
    assert!(
        codes("fn f(n: Int) -> Int decreases n decreases n = 0")
            .contains(&Code::ParseDuplicateClause)
    );
}

#[test]
fn bad_items_and_eof() {
    assert!(codes("let x = 1").contains(&Code::ParseBadItem));
    assert!(codes("fn f() -> Int =").contains(&Code::ParseUnexpectedEof));
    assert!(codes("fn f() -> Int = (1 + 2").contains(&Code::ParseUnclosedDelimiter));
    assert!(codes("fn f() -> Int = {\n  1").contains(&Code::ParseUnclosedDelimiter));
}

#[test]
fn expected_things() {
    assert!(codes("fn f() -> Int = )").contains(&Code::ParseExpectedExpr));
    assert!(codes("fn f(x: ) -> Int = 1").contains(&Code::ParseExpectedType));
    assert!(
        codes("fn f(x: Int) -> Int = match x {\n  => 1\n}").contains(&Code::ParseExpectedPattern)
    );
    assert!(codes("fn f( -> Int = 1").contains(&Code::ParseUnexpectedToken));
}

#[test]
fn discarded_expression_statement() {
    assert!(codes("fn f() -> Int = {\n  1\n  2\n}").contains(&Code::ParseUnexpectedToken));
}

#[test]
fn recovery_reports_errors_in_several_items() {
    let out = parse(
        "fn a() -> Int = )\nfn b() -> Int = 2\nfn c() -> Int = ]\ntype Ok1 = Int\n",
        ParseOptions::default(),
    );
    assert_eq!(out.problems.len(), 2, "{:#?}", out.problems);
    let names: Vec<_> = out
        .file
        .items
        .iter()
        .map(|i| match i {
            Item::Fn(f) => f.name.name.clone(),
            Item::Type(t) => t.name.name.clone(),
            _ => String::new(),
        })
        .collect();
    assert_eq!(names, vec!["b", "Ok1"]);
}

#[test]
fn recovery_inside_a_block_continues_with_next_statement() {
    let out = parse(
        "fn f() -> Int = {\n  let a = )\n  let b = ]\n  1\n}\n",
        ParseOptions::default(),
    );
    assert_eq!(out.problems.len(), 2, "{:#?}", out.problems);
}

#[test]
fn never_panics_on_garbage() {
    for src in [
        "",
        "fn",
        "fn f",
        "fn f(",
        "fn f()",
        "fn f() ->",
        "type",
        "type X =",
        "type X = |",
        "((((((",
        "}}}}",
        "fn f() -> Int = match",
        "fn f() -> Int = match x {",
        "fn f() -> Int = x =>",
        "\"unterminated",
        "fn f() -> Int = [1, 2",
        "test t =",
        "const c: = 1",
        "fn f() -> Int = a.",
        "fn f() -> Int = a.0.",
        "type X[ = Int",
        "fn f[A() -> Int = 1",
        "fn f() -> Int = if { } else { }",
    ] {
        let _ = parse(src, ParseOptions::default());
    }
}

#[test]
fn one_element_tuple_pattern_is_rejected() {
    // found by the mutation fuzzer: `(p,)` used to parse as a one-element tuple the formatter printed as `(p)`
    assert!(!codes("fn f(x: Bool) -> Int = match x {\n  (true,) => 1\n  _ => 0\n}").is_empty());
}

#[test]
fn brace_body_is_a_block_body() {
    let f = ok("fn f(n: Int) -> Int {\n  n + 1\n}");
    let Item::Fn(d) = &f.items[0] else {
        panic!("not a fn")
    };
    assert!(matches!(
        d.body.as_ref().map(|b| &b.kind),
        Some(ExprKind::Block { .. })
    ));
}

#[test]
fn return_is_explained() {
    assert!(!codes("fn f(n: Int) -> Int {\n  return n\n}").is_empty());
}
