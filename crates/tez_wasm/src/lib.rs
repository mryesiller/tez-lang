//! WebAssembly (WasmGC) code generation for checked Tez programs.
//!
//! Every Tez value is an `anyref`:
//!
//! | Tez | representation |
//! |---|---|
//! | `Int` | `i31ref` when it fits in 31 bits, otherwise a JS `BigInt` held as `anyref` |
//! | `Bool`, `Unit` | `i31ref` (0/1, 0) |
//! | `Float` | `struct $F { f64 }` |
//! | `Str` | a JS string |
//! | `List[T]` | a JS array (never mutated after creation) |
//! | tuple, record, sum type | `struct $ObjK { tag: i32, f0..f(K-1): anyref }` (a subtype chain) |
//! | function value | `struct $CloK { code: (ref $FnK), env: (ref null $Env) }` |
//!
//! The runtime (`runtime/tez-runtime.mjs`) implements the prelude's `extern fn`s and big-integer
//! arithmetic; this crate emits everything else. Programs are assumed to be *verified*: where the
//! language guarantees an operation cannot fail (division by zero, indexing), the emitted code
//! simply traps or throws if the guarantee is violated.

mod emit;

pub use emit::{CompileError, Module, compile};
