//! Formatter tests: golden outputs, idempotence over the prelude, comment safety.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use tez_fmt::{FmtError, format_source};
use tez_syntax::ParseOptions;

fn fmt(src: &str) -> String {
    format_source(src, ParseOptions::default()).unwrap_or_else(|e| panic!("{e:?}\n{src}"))
}

fn assert_fmt(src: &str, want: &str) {
    let got = fmt(src);
    assert_eq!(got, want, "\n--- got ---\n{got}\n--- want ---\n{want}");
    assert_eq!(fmt(&got), got, "not idempotent");
}

#[test]
fn simple_function_stays_on_one_line() {
    assert_fmt(
        "fn   add( a:Int ,b : Int)->Int=a+b\n\n\n",
        "fn add(a: Int, b: Int) -> Int = a + b\n",
    );
}

#[test]
fn items_are_separated_by_one_blank_line() {
    assert_fmt(
        "fn a() -> Int = 1\nfn b() -> Int = 2\n\n\n\nconst c: Int = 3\ntest t = a() == 1",
        "fn a() -> Int = 1\n\nfn b() -> Int = 2\n\nconst c: Int = 3\n\ntest t = a() == 1\n",
    );
}

#[test]
fn contracts_go_on_their_own_lines() {
    assert_fmt(
        "fn f(x:Int)->Int requires x>0 ensures result>=0 decreases x = x",
        "fn f(x: Int) -> Int\n  requires x > 0\n  ensures result >= 0\n  decreases x\n  = x\n",
    );
}

#[test]
fn types() {
    assert_fmt("type   P={x:Int,y:Int}", "type P = { x: Int, y: Int }\n");
    assert_fmt("type S = |A|B(Int,Str)", "type S = A | B(Int, Str)\n");
    assert_fmt(
        "type T[A]=Leaf|Node(T[A],A,T[A])",
        "type T[A] = Leaf | Node(T[A], A, T[A])\n",
    );
    assert_fmt("type Pos=Int where it>0", "type Pos = Int where it > 0\n");
    assert_fmt(
        "fn f(g:(Int,Int)->Int)->(Int,Str)=(1,\"a\")",
        "fn f(g: (Int, Int) -> Int) -> (Int, Str) = (1, \"a\")\n",
    );
}

#[test]
fn long_adt_breaks_into_bars() {
    let src = "type Token = Number(Int) | Identifier(Str) | Plus | Minus | Star | Slash | LParen | RParen | Comma | Semicolon | Eof";
    assert_fmt(
        src,
        "type Token =\n  | Number(Int)\n  | Identifier(Str)\n  | Plus\n  | Minus\n  | Star\n  | Slash\n  | LParen\n  | RParen\n  | Comma\n  | Semicolon\n  | Eof\n",
    );
}

#[test]
fn match_if_and_blocks() {
    assert_fmt(
        "fn f(o:Option[Int])->Int=match o{Some(n)if n>0=>n\nSome(_)=>0\nNone=>-1}",
        "fn f(o: Option[Int]) -> Int = match o {\n  Some(n) if n > 0 => n\n  Some(_) => 0\n  None => -1\n}\n",
    );
    assert_fmt(
        "fn f(n:Int)->Int=if n>0{1}else if n<0{-1}else{0}",
        "fn f(n: Int) -> Int = if n > 0 { 1 } else if n < 0 { -1 } else { 0 }\n",
    );
    assert_fmt(
        "fn f(x:Int)->Int={let y=x+1\nassert y>0\ny}",
        "fn f(x: Int) -> Int = {\n  let y = x + 1\n  assert y > 0\n  y\n}\n",
    );
}

#[test]
fn lambdas_records_and_patterns() {
    assert_fmt(
        "fn f(xs:List[Int])->List[Int]=xs.map(x=>x*2)",
        "fn f(xs: List[Int]) -> List[Int] = xs.map(x => x * 2)\n",
    );
    assert_fmt(
        "fn f(xs:List[Int])->Int=xs.fold(0,(a,b)=>a+b)",
        "fn f(xs: List[Int]) -> Int = xs.fold(0, (a, b) => a + b)\n",
    );
    assert_fmt(
        "fn f(p:P)->P=p with{x:1,y:2}",
        "fn f(p: P) -> P = p with { x: 1, y: 2 }\n",
    );
    assert_fmt(
        "fn f(xs:List[Int])->Int=match xs{[]=>0\n[a,..t]=>a\n}",
        "fn f(xs: List[Int]) -> Int = match xs {\n  [] => 0\n  [a, ..t] => a\n}\n",
    );
}

#[test]
fn long_lines_wrap() {
    let src = "fn f(first: Int, second: Int, third: Int, fourth: Int, fifth: Int, sixth: Int, seventh: Int) -> Int = first";
    let out = fmt(src);
    assert!(out.lines().all(|l| l.chars().count() <= 100), "{out}");
    assert!(out.contains("first: Int,\n"), "{out}");
    assert_eq!(fmt(&out), out);

    let chain = "fn f(xs: List[Int]) -> List[Int] = xs.filter(x => x > 0).map(x => x * 2).filter(x => x < 1000).map(x => x + 1).filter(x => x % 3 != 0).map(x => x * x).reverse()";
    let out = fmt(chain);
    assert!(out.lines().all(|l| l.chars().count() <= 100), "{out}");
    assert!(out.contains("\n    .map("), "{out}");
    assert_eq!(fmt(&out), out);

    let cond = "fn f(a: Bool, b: Bool, c: Bool) -> Bool = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb && cccccccccccccccccccccccccccccccccccc";
    let out = fmt(cond);
    assert!(out.contains("\n    && "), "{out}");
    assert_eq!(fmt(&out), out);
}

#[test]
fn strings_are_escaped_canonically() {
    assert_fmt(
        "fn f() -> Str = \"a\\nb\\t\\\"q\\\" \\u{41}\"",
        "fn f() -> Str = \"a\\nb\\t\\\"q\\\" A\"\n",
    );
}

#[test]
fn comments_are_kept() {
    assert_fmt(
        "// header\nfn a() -> Int = 1 // trailing\n\n// about b\nfn b() -> Int = {\n  // first\n  let x = 1 // one\n  // before tail\n  x\n}\n",
        "// header\nfn a() -> Int = 1 // trailing\n\n// about b\nfn b() -> Int = {\n  // first\n  let x = 1 // one\n  // before tail\n  x\n}\n",
    );
    assert_fmt(
        "fn f(x: Int) -> Int = match x {\n  // zero\n  0 => 1 // yes\n  _ => 2\n}\n",
        "fn f(x: Int) -> Int = match x {\n  // zero\n  0 => 1 // yes\n  _ => 2\n}\n",
    );
    assert_fmt(
        "fn a() -> Int = 1\n\n// the end\n",
        "fn a() -> Int = 1\n\n// the end\n",
    );
}

#[test]
fn comments_in_expressions_are_refused_not_dropped() {
    let src = "fn f() -> Int = 1 + // inner\n  2\n";
    assert!(matches!(
        format_source(src, ParseOptions::default()),
        Err(FmtError::Comment(_))
    ));
}

#[test]
fn syntax_errors_are_reported() {
    assert!(matches!(
        format_source("fn f( -> Int = 1", ParseOptions::default()),
        Err(FmtError::Syntax(_))
    ));
}

#[test]
fn prelude_is_canonical_and_idempotent() {
    let src = include_str!("../../tez_types/prelude/prelude.tez");
    let once = format_source(src, ParseOptions { prelude: true }).unwrap();
    let twice = format_source(&once, ParseOptions { prelude: true }).unwrap();
    assert_eq!(once, twice, "prelude formatting is not idempotent");
}

#[test]
fn empty_input() {
    assert_eq!(fmt(""), "");
    assert_eq!(fmt("\n\n"), "");
}
