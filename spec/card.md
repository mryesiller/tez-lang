# Tez in one page
Pure, typed, verified. No null, exceptions, loops, mutation, classes. Names: `lowerCamel` values, `UpperCamel` types. 2-space indent, no `;`.

## Types
`Int` (unbounded) `Float` `Bool` `Str` `()` `List[T]` `(A, B)` `(A) -> B` `Option[T]`=`None|Some(T)` `Result[T,E]`=`Ok(T)|Err(E)`
```
type Point = { x: Int, y: Int }
type Shape = Circle(Int) | Rect(Int, Int)
type Pos = Int where it > 0
```

## Functions: signature always written; body is one expression after `=`
```
fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Rect(w, h) => w * h
}
fn div(a: Int, b: Int where it != 0) -> Int = a / b
fn avg(xs: List[Int]) -> Int
  requires xs.len() > 0
  = xs.sum() / xs.len()
fn clamp(x: Int, lo: Int, hi: Int) -> Int
  requires lo <= hi
  ensures lo <= result && result <= hi
  = if x < lo { lo } else if x > hi { hi } else { x }
fn fact(n: Int) -> Int
  requires n >= 0
  decreases n
  = if n == 0 { 1 } else { n * fact(n - 1) }
```
Generic: `fn first[T](xs: List[T]) -> Option[T]`.

## Expressions
- `if c { a } else { b }` (else required) · `match e { pat => v }` one arm per line, no commas
- block: `{ let y = f(x) \n assert y > 0 \n y + 1 }` (last line is the value)
- `Point { x: 1, y: 2 }` · `p with { x: 3 }` · `p.x` · `t.0` · `xs[i]` · `[1, 2]` · `(a, b)`
- lambda `x => x + 1`, `(a, b) => a + b`, `_ => 0`
- `a ++ b` joins Str · `&&` and `||` need parentheses when mixed · comparisons do not chain
- `/` and `%` truncate toward zero · patterns: `_` `n` `0` `Some(x)` `(a, b)` `[]` `[h, ..t]`, guard `n if n > 0`

## Methods: always `value.method(args)`, never `List.map(xs, f)`
- Int `abs min max clamp pow isEven isOdd toStr toFloat` · Float `sqrt floor ceil round abs min max toStr`
- Str `len slice chars toInt contains startsWith endsWith trim upper lower split repeat isEmpty`
- List `len get map filter fold flatMap append concat reverse take drop any all count contains indexOf first last zip enumerate sort sortBy sum min max join isEmpty` · static `List.range(lo, hi)` `List.repeat(x, n)`
- Option `isSome isNone unwrapOr map andThen` · Result `isOk unwrapOr map mapErr toOption`
- UI view: `Html[Msg]` built with `Html.text(s)` `Html.heading(s)` `Html.paragraph(s)` `Html.button(label, msg)` `Html.input(value, f)` `Html.row(kids)` `Html.col(kids)` `Html.node(tag, attrs, kids)`

## What the compiler proves (else it rejects with a code and a counterexample)
Division and `%` need a nonzero divisor; `xs[i]` and `get` need `0 <= i < len`; every call must meet the callee's `requires`; `ensures` and `where` must hold; a recursive function needs `decreases` (an Int that shrinks, or a data value that gets smaller). State what a caller needs in `requires`, what it gets in `ensures`.
Don't reuse a name already in scope. Don't write `snake_case`. A `test name = expr` item checks a Bool.
