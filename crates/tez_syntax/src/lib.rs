//! Syntax front end for Tez: source positions, problems, lexer, AST and parser.

pub mod ast;
mod lexer;
mod names;
mod parser;
mod problem;
mod source;

pub use lexer::{Comment, Kw, LexOutput, Tok, Token, lex};
pub use names::{is_lower_camel, is_upper_camel, to_lower_camel, to_upper_camel};
pub use parser::{ParseOptions, ParseOutput, parse};
pub use problem::{Problem, ProblemFix};
pub use source::{ByteSpan, LineIndex};
