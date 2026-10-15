# A tour of Tez

Tez is small on purpose. This tour shows nearly everything in about ten minutes. Every block marked as a program has a **Run** button on the website, which compiles it to WebAssembly in your browser. The same blocks are compiled and run by the test suite, so they cannot rot.

## 1. Hello

A program is a list of items. `main` is the entry point: a function with no parameters. Everything is an expression, so the function body is `= expression`.

```tez
fn main() -> Str = "Hello, " ++ "Tez"
```

There are no statements, no loops, no `null`, no exceptions and no mutable variables. That is what makes the guarantees later in this tour possible.

## 2. Functions and types

Every function states its parameter and return types in full. Names are `lowerCamel` for values and `UpperCamel` for types.

```tez
type Shape = Circle(Int) | Rect(Int, Int)

fn area(s: Shape) -> Int = match s {
  Circle(r) => 3 * r * r
  Rect(w, h) => w * h
}

fn main() -> Int = area(Rect(4, 5)) + area(Circle(2))
```

`Shape` is a sum type. `match` must cover every variant: leave one out and the program is rejected (see section 11).

## 3. Records

```tez
type Point = { x: Int, y: Int }

fn moveRight(p: Point, dx: Int) -> Point = p with { x: p.x + dx }

fn main() -> Point = moveRight(Point { x: 1, y: 2 }, 10)
```

Records are immutable. `with` builds a copy with some fields replaced.

## 4. Lists and methods

Built-in types have methods, always called with the dot: `xs.map(f)`. There is exactly one way to call them, so a model never has to choose between `map(xs, f)` and `xs.map(f)`.

```tez
fn main() -> Int =
  List.range(1, 11)
    .filter(n => n % 2 == 0)
    .map(n => n * n)
    .sum()
```

Lambdas are `x => expression` or `(a, b) => expression`.

## 5. Absence and failure are values

There is no `null` and no exception. A value that might be missing is an `Option`; an operation that might fail is a `Result`.

```tez
fn safeDiv(a: Int, b: Int) -> Result[Int, Str] =
  if b == 0 { Err("divide by zero") } else { Ok(a / b) }

fn main() -> Int = safeDiv(10, 0).unwrapOr(-1) + safeDiv(10, 2).unwrapOr(-1)
```

Inside `else`, the compiler knows `b != 0`, so the division `a / b` is accepted. Write `a / b` where it cannot know that and the program is rejected with a counterexample.

## 6. Refinement types

A type can carry a condition. `Int where it > 0` is the positive integers, and `it` names the value.

```tez
type Pos = Int where it > 0

fn half(n: Pos) -> Int = n / 2

fn main() -> Int = half(10)
```

`half(0)` does not compile: the compiler must prove every argument satisfies `it > 0`. Records can carry refinements too, and the verifier keeps them true through every update. The counter in section 10 uses `count: Int where it >= 0`.

## 7. Contracts

`requires` states what a function needs, `ensures` what it promises about `result`. Callers are checked against `requires`; the body is checked against `ensures`.

```tez
fn clampPercent(n: Int) -> Int
  ensures 0 <= result && result <= 100
  = if n < 0 { 0 } else if n > 100 { 100 } else { n }

fn main() -> Int = clampPercent(250)
```

Out-of-range indexing is a contract too. `xs[i]` and `xs.get(i)` require `0 <= i < xs.len()`:

```text
error[E_INDEX_BOUNDS]: the index may be out of range
  --> a.tez:1:39
  |
1 | fn at(xs: List[Int], i: Int) -> Int = xs.get(i)
  |                                       ^^^^^^^^^
   = counterexample: i = 1, xs.len() = 1
```

The SMT proofs run in the `tez` command line and, in the Studio, behind the **Verify** button (Z3 compiled to WebAssembly). The compiler on this website checks syntax, names, types and exhaustiveness.

## 8. Recursion must end

There are no loops, so repetition is recursion, and a recursive function must say what shrinks with `decreases`. The compiler proves it gets smaller on every call.

```tez
fn sum(xs: List[Int]) -> Int
  decreases xs.len()
  = match xs {
    [] => 0
    [head, ..tail] => head + sum(tail)
  }

fn main() -> Int = sum([1, 2, 3, 4, 5])
```

Patterns can take lists apart: `[]`, `[a, b]`, `[head, ..tail]`.

## 9. Tests

`test name = expression` items are checked by `tez test`. They are not part of the compiled program.

```tez
fn double(n: Int) -> Int = n * 2

test doublesZero = double(0) == 0

test doublesNegatives = double(-4) == -8

fn main() -> Int = double(21)
```

## 10. Apps

A program with `init`, `update` and `view` is an interactive app, in the style of The Elm Architecture. `view` returns `Html[Msg]`; the runtime draws it and sends the `Msg` of a clicked button back to `update`. Because `update` is an ordinary verified function, `count: Int where it >= 0` holds in every state the user can reach.

```tez
type Model = { count: Int where it >= 0 }

type Msg = Inc | Dec

fn init() -> Model = Model { count: 0 }

fn update(m: Model, msg: Msg) -> Model = match msg {
  Inc => m with { count: m.count + 1 }
  Dec => if m.count > 0 { m with { count: m.count - 1 } } else { m }
}

fn view(m: Model) -> Html[Msg] =
  Html.row([Html.button("-", Dec), Html.paragraph(m.count.toStr()), Html.button("+", Inc)])
```

Time is an input too. An app may define `fn subs(m: Model) -> List[Sub[Msg]]`; `Sub.every(1000, Tick)` makes the runtime send `Tick` every second for as long as the model lists it. So a timer is just another event for the pure `update`, and the compiler proves the interval is at least 50 ms. Example 49 (a traffic light that changes colour by itself, with different durations per light) and 41 (a countdown with Start/Pause) are written this way.

Edit an app while it runs in the playground and the running session is kept: the runtime replays your clicks into the new program.

## 11. What gets rejected

Mistakes a small model makes are stopped with a stable code and a precise message. Each snippet below is compiled by the website, and the test suite checks that it fails with exactly the named code.

```tez-reject
// expect: E_MATCH_NON_EXHAUSTIVE
type Light = Red | Yellow | Green

fn next(l: Light) -> Light = match l {
  Red => Green
  Green => Yellow
}

fn main() -> Light = next(Red)
```

```tez-reject
// expect: E_NAME_SHADOWING
fn main() -> Int = {
  let x = 1
  let x = 2
  x
}
```

```tez-reject
// expect: E_PARSE_NAME_CASE
fn main() -> Int = {
  let my_value = 1
  my_value
}
```

Diagnostics are available as JSON with spans, the expected form and machine-applicable fixes, so a model (or an editor) can repair a program without guessing. See the [error catalog](#/errors).
