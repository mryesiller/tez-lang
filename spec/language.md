# Tez language reference v0.1

Companion to [`grammar.ebnf`](grammar.ebnf) (syntax) and [`errors.md`](errors.md) (diagnostics). A program is a list of items in one file. Everything is an expression; everything is pure and immutable.

## 1. Lexical rules

- Source is UTF-8. Whitespace is spaces and newlines; tabs are an error (`E_LEX_UNEXPECTED_CHAR`).
- `// comment` runs to the end of the line.
- Names: `lowerCamel` for values, functions, fields, parameters; `UpperCamel` for types, variants, type parameters and built-in modules.
- Keywords (reserved): `fn type const test use let match if else where with requires ensures decreases assert extern true false`. `it` and `result` are not keywords: they are bound only inside refinements and `ensures`.
- Int: `123`, `1_000` (arbitrary precision). Float: `1.5` (digits both sides of the dot). Str: `"text"` with escapes `\n \t \r \\ \" \0 \u{1F600}`.
- Newlines separate items, statements and match arms. They are ignored inside `( )` and `[ ]`. A line continues if it begins with `.`, `&&` or `||`, or the previous line ends with a binary operator, `(`, `[`, `{`, `,` or `=>`.

## 2. Types

| Type | Notes |
|---|---|
| `Int` | arbitrary-precision integer; overflow does not exist |
| `Float` | IEEE-754 f64; NaN and infinity cannot arise in verified programs |
| `Bool`, `Str`, `Unit` | `()` is the only Unit value; Str is Unicode text |
| `List[T]` | immutable, indexable sequence |
| `(A, B, ..)` | tuple, two or more elements, fields `.0 .1 ..` |
| `(A, B) -> C` | function type; `() -> C` takes nothing |
| `Option[T]` | `None` or `Some(T)` |
| `Result[T, E]` | `Ok(T)` or `Err(E)` |
| `type P = { x: Int, y: Int }` | record (nominal) |
| `type S = A \| B(Int, Str)` | sum type / ADT; variants are positional |
| `type T[A] = ..` | generic type |
| `type Pos = Int where it > 0` | alias; with `where` it is a **refined** type |

There is no `null`, no exceptions, no mutable variables, no loops, no classes, no inheritance, no macros, no overloading, no implicit conversion, no default arguments. Types never convert silently: use `n.toFloat()`, `x.floor()`.

### Refinements

`T where P` denotes the values of `T` for which `P` (a `Bool` expression, with `it` naming the value) holds. Refinements may appear on parameters, return types, `let` annotations, record fields, variant payloads and alias bodies, but **only at the top of a type** (not inside `List[..]`, tuples or function types: `E_TYPE_REFINEMENT_POSITION`). A refined type is a subtype of its base type; the reverse direction creates an obligation (see §7).

## 3. Declarations

```tez
type Shape = Circle(Int) | Rect(Int, Int)       // sum type
type Point = { x: Int, y: Int }                 // record
type Pos = Int where it > 0                     // refined alias
const limit: Int = 100                          // constant, evaluated once
fn area(s: Shape) -> Int = match s { ... }      // function: `= expression` is the body
test areaWorks = area(Rect(2, 3)) == 6          // checked by `tez test`, excluded from the guarantee
```

- Every function has a full signature: all parameters and the return type are annotated. Generic functions list type parameters: `fn first[A](xs: List[A]) -> Option[A]`.
- Clauses go between the return type and `=`: `requires`, `ensures` (may use `result`), `decreases`. Several `requires`/`ensures` are conjoined.
- Order of items does not matter. Names must be unique (`E_NAME_DUPLICATE`) and must not collide with the prelude (`E_NAME_PRELUDE`).
- A binding may not reuse a name already in scope (`E_NAME_SHADOWING`), including parameters, `let`, lambda parameters and pattern bindings.
- Recursive and mutually recursive functions require `decreases` (§7), with one exception: a self-recursive function whose every recursive call passes, for one parameter, the tail of a `[h, ..t]` match on it (or a same-typed part of a matched constructor) gets the measure `p.len()` (or `p`) inferred; the verifier still proves it. Constants may not be recursive.
- `fn f(..) -> T { .. }` is accepted as `fn f(..) -> T = { .. }`; `tez fmt` writes the `=`.

### 3.1 Files and `use`

A program may span several files. `use "geometry.tez"` (a top-level item, usually first) makes every item of that file part of this program and visible in this file.

- The path is relative to the file that contains the `use`, ends in `.tez`, uses `/` separators and must stay inside the directory of the entry file (no absolute paths, no `..` above it). Violations are `E_USE_BAD_PATH`; a missing file is `E_USE_NOT_FOUND`.
- A file is loaded once however many files use it. Imports may not form a cycle (`E_USE_CYCLE`). Order does not matter.
- Names are unqualified and unique across the whole program: a name defined twice, in any two files, is `E_NAME_DUPLICATE`, and nothing can shadow anything (the same rules as inside one file). Only the entry file should define `main`.
- A file sees only its own items and those of the files it `use`s **directly**; using anything else (a function, constant, type or constructor) is `E_USE_MISSING`, with a fix that adds the `use` line. Importing is not transitive: if `main.tez` uses `cart.tez` and `cart.tez` uses `money.tez`, `main.tez` must `use "money.tez"` itself to call `format`. Values whose type comes from another file can still be passed around and their fields read without naming the type. When the name's file already imports this one, the `use` would be a cycle: move the item instead.
- `tez check`, `run`, `test` and `build` take the entry file and follow the imports; everything is verified together, so a contract in one file is proved against its uses in another. `tez test` runs the `test` items of every file. Diagnostics name the file they are in (`file` in the JSON report). `tez fmt` formats one file at a time.
- `use` is not available where there are no other files (the single-file editor in a browser): it reports `E_USE_NOT_FOUND`.

## 4. Expressions

| Form | Meaning |
|---|---|
| `f(a, b)` | call a function or a function-typed value |
| `xs.map(f)` | **method call**: the receiver is the first argument of the built-in `List.map`. Methods exist only for built-in types |
| `List.range(0, n)` | static built-in (no receiver) |
| `p.x`, `t.0` | record field, tuple field |
| `xs[i]` | element access; creates an obligation `0 <= i < xs.len()` |
| `Point { x: 1, y: 2 }` | record literal (all fields, once each) |
| `p with { x: 3 }` | copy of `p` with fields replaced |
| `Circle(5)`, `None` | constructor application / nullary constructor |
| `[1, 2]`, `(a, b)`, `()` | list, tuple, unit |
| `x => x + 1`, `(a, b) => a + b` | lambda; parameter types are inferred or annotated: `(a: Int) => ..` |
| `if c { a } else { b }` | conditional; `else` is mandatory; both branches have the same type |
| `match e { .. }` | pattern match, arms on separate lines, no commas |
| `{ let y = 1 \n assert y > 0 \n y + 1 }` | block: `let`/`assert` statements then a final expression |
| `a ++ b` | Str concatenation (Str only) |

Operators by precedence, loosest first: `||`; `&&`; `== != < <= > >=` (non-associative); `++`; `+ -`; `* / %`; unary `- !`; postfix. Mixing `&&` and `||` needs parentheses.

- `==`/`!=` compare any two values of the same type structurally, except types containing functions. `<` etc. work on `Int`, `Float`, `Str` only.
- `/` and `%` on `Int` truncate toward zero (`-7 / 2 == -3`, `-7 % 2 == -1`); the divisor must be provably nonzero. `Float` `/` needs a provably nonzero divisor.
- `&&` and `||` short-circuit.
- A method call picks `Module.name` from the **receiver's type** (`Int`, `Float`, `Str`, `List`, `Option`, `Result`). The receiver's type must already be known (`E_TYPE_AMBIGUOUS` otherwise). Writing `List.map(xs, f)` is `E_NAME_USE_METHOD_SYNTAX`.

### Patterns

`_`, a name, a literal (`0`, `-1`, `"a"`, `true`), a constructor (`Some(x)`, `Rect(w, h)`), a tuple `(a, b)`, a list `[]`, `[a, b]`, `[h, ..t]`, `[.., last]` is **not** allowed (the rest pattern is last). Guards: `n if n > 0 => ..`. A `match` must be exhaustive and have no unreachable arm. Guarded arms do not count toward exhaustiveness.

## 5. Evaluation

Strict, left to right, call-by-value. `if`/`match`/`&&`/`||` evaluate only the taken branch. No effects exist in v0.1; `tez run` evaluates a zero-argument function and prints its value.

## 6. Prelude

The built-in types and functions are declared in Tez itself (`crates/tez_types/prelude/prelude.tez`), with contracts. `extern fn` declares a function implemented by the runtime; its contract is trusted. Methods are declared with a first parameter named `self`.

## 7. Guarantees and verification

If `tez check` reports no diagnostics, then at run time none of these can happen: type errors, missing match arms, null/undefined, uncaught exceptions, data races or shared mutable state, undefined names, integer overflow, division by zero, float NaN/infinity, out-of-range indexing, violation of any `requires`, `ensures`, refined type or `assert`, non-termination. Resource limits remain and are reported as `E_RESOURCE`: memory, stack, very large lists or strings, and `Float` overflow (a result beyond ±1.8e308 stops the program instead of becoming infinite, so NaN and infinity never exist; `Float` literals must be finite). The prelude's trusted contracts are checked against Unicode text (e.g. no length contract on `upper`/`lower`, because `"ß".upper()` is `"SS"`). The proof cache (`~/.cache/tez/vc`) is trusted like the compiler itself: whoever can write to it can mark a query as proved; set `TEZ_NO_CACHE` where that is not acceptable. Everything the solver cannot prove is rejected (`E_UNPROVEN`).

**Obligations** are generated at: `/` and `%` (divisor ≠ 0); Float `/` and `sqrt`; indexing and `List.get`-style built-ins (their `requires`); every call (callee `requires` and refined parameter types); every function exit (`ensures` and refined return type); `let` with a refined annotation; constructing a record or variant with refined fields; `assert`; recursive calls (`decreases`).

**Facts** the solver may use: parameter refinements and `requires`; path conditions from `if` and `match` arms; the contracts of called functions (`ensures`, refined results); values of non-recursive functions are unfolded when expressible; `let` bindings; field refinements when a field is read.

**Termination**: `decreases e`, where `e` is an `Int` or a data value (a record or sum type). For an `Int`, at every recursive call within the cycle `e` must be ≥ 0 and strictly smaller than at entry (arguments substituted); list recursion typically uses `decreases xs.len()`. For a data value the measure is its structural size: the call must receive a proper sub-value (`decreases t` for `Node(l, x, r) => f(l)`). A recursive function's own definition is available to the solver only after its termination is proved.

## 8. Entry points and tests

`tez run file.tez [fn]` verifies the program, then evaluates the zero-argument function `fn` (default `main`) and prints its value. `--no-verify` skips verification (no guarantee), `--contracts` evaluates contracts at run time. `test name = <Bool expr>` items are evaluated by `tez test`; tests are not part of the compiled program and not verified.

## 8.1 Apps (UI)

A program with `fn init() -> Model`, `fn update(m: Model, msg: Msg) -> Model` and `fn view(m: Model) -> Html[Msg]` is an app. The host (`runtime/tez-app.mjs`) renders only inert HTML: script-capable tags (`script`, `iframe`, `object`, `style`, `form`, `svg`, …) become `div`, `on*` attributes and `srcdoc` are dropped, and `href`/`src` accept only `http(s):`, `mailto:` and relative links, so a program cannot run JavaScript in the page. It calls `view` after every `update` and patches only the DOM that changed. `Html[Msg]` is built with `Html.text`, `Html.heading`, `Html.paragraph`, `Html.button(label, msg)`, `Html.input(value, f)`, `Html.row`, `Html.col` and `Html.node`. `Sub.every(ms, msg)` is how time enters an app: an optional `fn subs(m: Model) -> List[Sub[Msg]]` lists the subscriptions that are active for a model, and the host sends `msg` every `ms` milliseconds (at least 50) while the subscription is listed; when the model changes so that it is no longer listed, the timer stops. Time is therefore just another event for the pure `update`. Three more subscriptions carry an argument: `Sub.clock(ms, f)` (`ms` ≥ 1000) sends `f(t)` where `t` is the wall clock in seconds since 1970-01-01 UTC, so a schedule ("once a minute", "at 09:00") is ordinary pure code comparing `t` with the model; `Sub.online(f)` and `Sub.visible(f)` send `f(true)`/`f(false)` when the browser goes online/offline or the page becomes visible/hidden. The host logs the argument with each tick, so hot-swap replays a session exactly (`spec/examples/55-scheduled-page.tez`).

**Network and memory, also declared (`spec/examples/56-saved-quote.tez`).** An optional `fn fetches(m: Model) -> List[Fetch[Msg]]` lists GET requests, `Fetch.get(url, onOk, onErr)`: each time a url newly appears in the list the host fetches it once and sends `onOk(body)` or `onErr(reason)`; the program stops waiting by removing it from the list (a flag in the model). The page's owner decides what can be reached: relative URLs resolve against the page, and only the page's own origin plus origins given to the host (`tez site --allow=https://api.example`) are allowed (no cookies, no referrer, no redirects, 10 s, 1 MB); anything else answers `onErr`. Optional `fn save(m: Model) -> Str` and `fn restore(s: Str) -> Option[Model]` keep the model in this browser: after each change the host stores `save(m)`, and at start it tries `restore` on what is stored (`None` or an unreadable state starts from `init()`). `restore` builds the model in ordinary verified code, so refinements on `Model` hold for restored state too. Constructors are not functions, so write `s => Typed(s)`, not `Typed`. `update` and `view` are ordinary verified functions: refinements on `Model` hold in every state.

## 9. What the verifier knows

The verifier is sound, not complete: a correct program can be rejected (`E_UNPROVEN`) when the solver cannot find the proof. What it reasons about:

- **Integers** exactly (linear arithmetic; non-linear terms may be too hard), with truncating `/` and `%`.
- **Lists** through length and element access, including `append`, `take`, `drop`, `concat`, `reverse`, `range`, `repeat`; patterns `[]`, `[h, ..t]`.
- **Strings** through their length; **floats** through ordering and the sign of differences only (no rounding arithmetic).
- **Records, sum types, tuples** structurally, with field refinements assumed when a field is read and proved when a value is built.
- **Calls**: the callee's `requires` is proved at the call, its `ensures` is assumed afterwards. Non-recursive functions are unfolded; recursive ones contribute their definition once proved to terminate.
- **Lambdas** passed to `map`, `filter`, `any`, `all`, `count`, `flatMap`, `sortBy`, `fold`: the parameter is known to be an element of the list. A named function passed the same way is checked for an arbitrary element.
- Not modelled: what `map`/`filter` return beyond their length, and `sort` beyond its length. State what you need in a contract on your own function.
