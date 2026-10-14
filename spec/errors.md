# Tez error catalog v1

Every diagnostic has a **stable code**. Codes are never renamed or reused. All problems are errors: Tez has no warnings, because a warning an agent may ignore is a defect that ships. A program is accepted only with zero diagnostics.

`phase` is the compiler phase that reports it. Verification codes are reported with `phase: verify` at compile time and with `phase: runtime` when the interpreter detects the same violation while running an unverified program.

## Lexing and parsing (phase `parse`)

| Code | Meaning | Typical fix |
|---|---|---|
| `E_LEX_UNEXPECTED_CHAR` | A character that cannot start any token | delete it |
| `E_LEX_UNTERMINATED_STRING` | String literal not closed on the same line | add the closing `"` |
| `E_LEX_BAD_ESCAPE` | Unknown escape such as `\q` | use `\n \t \r \\ \" \0 \u{..}` |
| `E_LEX_BAD_NUMBER` | Malformed number (`1__0`, `1_`, `1.` , `0x1`) | write plain digits, `1_000`, `1.5` |
| `E_PARSE_UNEXPECTED_TOKEN` | Token not allowed here; message states what was expected | follow `expected` |
| `E_PARSE_UNEXPECTED_EOF` | Input ended inside a construct | complete the construct |
| `E_PARSE_UNCLOSED_DELIMITER` | `(` `[` `{` never closed | add the closer |
| `E_PARSE_EXPECTED_EXPR` | An expression is required | write one |
| `E_PARSE_EXPECTED_TYPE` | A type is required | write one |
| `E_PARSE_EXPECTED_PATTERN` | A pattern is required | write one |
| `E_PARSE_MIXED_LOGIC` | `&&` and `\|\|` mixed without parentheses | parenthesize |
| `E_PARSE_CHAINED_COMPARISON` | `a < b < c` | use `a < b && b < c` |
| `E_PARSE_BLOCK_NO_TAIL` | Block does not end with an expression | add the result expression |
| `E_PARSE_ELSE_REQUIRED` | `if` without `else` | add `else { .. }` |
| `E_PARSE_STRAY_SEPARATOR` | `;` after a statement or `,` between match arms | delete it |
| `E_PARSE_RESERVED_WORD` | Keyword used as a name, or `extern` in user code | rename |
| `E_PARSE_NAME_CASE` | Type/variant/module name not UpperCamel, or value/field/function name not lowerCamel | rename |
| `E_PARSE_DUPLICATE_CLAUSE` | Second `decreases`, or `requires`/`ensures` after the body `=` | merge or move |
| `E_PARSE_BAD_ITEM` | Top level must contain `type`, `fn`, `const` or `test` | remove or fix |

## Names (phase `resolve`)

| Code | Meaning | Typical fix |
|---|---|---|
| `E_NAME_UNKNOWN` | Unknown variable or function | define it or fix the spelling |
| `E_NAME_UNKNOWN_TYPE` | Unknown type | define it or fix the spelling |
| `E_NAME_UNKNOWN_CTOR` | Unknown constructor | define it or fix the spelling |
| `E_NAME_UNKNOWN_METHOD` | The receiver's type has no such method | see `expected` for close names |
| `E_NAME_DUPLICATE` | Item, parameter, field, variant or pattern binding defined twice | rename one |
| `E_NAME_SHADOWING` | A `let`, parameter or pattern binding reuses a name already in scope | pick a new name |
| `E_NAME_PRELUDE` | Redefining a built-in type, constructor or module | rename |
| `E_NAME_USE_METHOD_SYNTAX` | `List.map(xs, f)` written instead of `xs.map(f)` | the `fixes` entry rewrites it |
| `E_CONTRACT_SCOPE` | `result` outside `ensures`, or `it` outside a refinement | move it |

## Types (phase `types`)

| Code | Meaning | Typical fix |
|---|---|---|
| `E_TYPE_MISMATCH` | Expected and found types differ | convert or fix the expression |
| `E_TYPE_ARITY` | Wrong number of arguments, type arguments, tuple or constructor fields | match the signature |
| `E_TYPE_NOT_FUNCTION` | Calling a value that is not a function | remove the call |
| `E_TYPE_UNKNOWN_FIELD` | Record has no such field, or the receiver is not a record | see `expected` |
| `E_TYPE_MISSING_FIELD` | Record literal omits a field | add it |
| `E_TYPE_EXTRA_FIELD` | Record literal repeats or invents a field | remove it |
| `E_TYPE_AMBIGUOUS` | A type cannot be inferred (`[]` alone, unknown method receiver) | annotate with `: T` |
| `E_TYPE_INFINITE` | A type would contain itself (occurs check) | restructure |
| `E_TYPE_NOT_BOOL` | `if` condition, guard, `assert`, contract or refinement is not `Bool` | make it Bool |
| `E_TYPE_BAD_OPERAND` | Operator applied to a type it does not support | use a supported type |
| `E_TYPE_NOT_EQUATABLE` | `==`/`!=` on a type containing functions | compare data instead |
| `E_TYPE_NOT_ORDERED` | `<` `<=` `>` `>=` on a type other than Int, Float, Str | compare Int/Float/Str |
| `E_TYPE_TUPLE_INDEX` | `t.N` where N is out of range or `t` is not a tuple | fix the index |
| `E_TYPE_REFINEMENT_POSITION` | A refinement nested inside `List[..]`, `Option[..]`, tuples or function types | refine the container with a contract |
| `E_TYPE_RECURSIVE_ALIAS` | An alias refers to itself without a variant or record | make it an ADT |
| `E_MATCH_NON_EXHAUSTIVE` | A `match` does not cover every value; `expected` lists a missing pattern | add the arm |
| `E_MATCH_UNREACHABLE` | An arm can never be reached | delete it |
| `E_MATCH_BINDING_MISMATCH` | The same name is bound twice in one pattern | rename |

## Verification (phase `verify`, also `runtime` in the interpreter)

| Code | Meaning | Typical fix |
|---|---|---|
| `E_DIV_ZERO` | The divisor of `/` or `%` may be 0 | add `requires b != 0` or a refinement |
| `E_FLOAT_DOMAIN` | Float division by 0.0, or `sqrt` of a negative | constrain the argument |
| `E_INDEX_BOUNDS` | An index may be out of range | add `requires 0 <= i && i < xs.len()` |
| `E_PRECONDITION` | A call may violate the callee's `requires` | establish it before the call |
| `E_POSTCONDITION` | The body may violate an `ensures` | fix the body or weaken the contract |
| `E_REFINEMENT` | A value may violate its refined type (`where`) | establish the predicate |
| `E_ASSERT` | An `assert` may fail | prove it or fix the code |
| `E_TERMINATION_MISSING` | A recursive function has no `decreases` | add `decreases <Int expr>` |
| `E_TERMINATION` | The `decreases` measure is not proven to be ≥ 0 and strictly smaller at a recursive call | change the measure or add a precondition |
| `E_UNPROVEN` | The solver can neither prove nor refute an obligation (unknown/timeout). **Rejected** | add facts the solver needs; the message names the obligation |

A refuted obligation carries a `counterexample` (concrete values). An `E_UNPROVEN` obligation has none.

## Runtime and tools

| Code | Meaning |
|---|---|
| `E_RESOURCE` | Fuel, memory or stack limit hit (the only run-time failure verified programs can have) |
| `E_TEST_FAILED` | A `test` evaluated to `false` (`tez test`) |
| `E_INTERNAL` | Compiler bug. Never a user error; report it |

## Rules for diagnostics

1. One short templated `message`; no prose paragraphs.
2. `span` points at the smallest construct that is wrong.
3. `expected` states what the compiler needed, in Tez syntax, when that is meaningful.
4. `fixes` is present whenever a safe mechanical edit exists; edits replace `span` with text.
5. The reporting phase continues after an error, so one run lists every independent problem. Later phases run only if earlier ones had no errors in the same item, to avoid cascades.
