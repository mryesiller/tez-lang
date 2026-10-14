//! Name resolution and type checking for Tez.
//!
//! [`check`] turns a parsed file (plus the embedded prelude) into a typed [`hir::Program`]
//! and a list of [`Problem`]s. A program is acceptable only when there are no problems.

mod check;
mod exhaust;
pub mod hir;
mod ty;

pub use check::{CheckOutput, check};
pub use ty::{Ty, TyDisplay, TypeId};

/// Source text of the prelude (built-in types and functions).
pub const PRELUDE_SRC: &str = include_str!("../prelude/prelude.tez");
