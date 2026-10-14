//! Canonical formatter for Tez.
//!
//! There is exactly one correct layout for a program. The formatter is idempotent
//! (`fmt(fmt(x)) == fmt(x)`) and refuses, rather than drops, comments it cannot keep.

mod doc;
mod format;

pub use format::{FmtError, format_file, format_source};
