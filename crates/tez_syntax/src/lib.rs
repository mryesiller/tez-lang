//! Syntax front end for Tez.
//!
//! Phase 0 only needs source positions; the lexer and parser arrive in Phase 1.

mod source;

pub use source::{ByteSpan, LineIndex};
