//! Structured diagnostics for Tez.
//!
//! Every compiler phase reports problems as [`Diagnostic`] values. The JSON form of a
//! [`Report`] is the contract between the compiler and LLM agents / editors, so it is
//! versioned ([`SCHEMA_VERSION`]) and kept deliberately small: absent optional fields
//! are omitted, which saves tokens in an LLM context.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Version of the JSON report schema. Bump on any breaking change.
pub const SCHEMA_VERSION: u32 = 1;

/// A source position. Both fields are 1-based; `col` counts Unicode scalar values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pos {
    /// 1-based line number.
    pub line: u32,
    /// 1-based column, counted in Unicode scalar values.
    pub col: u32,
}

/// A half-open source range `[start, end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// First position inside the range.
    pub start: Pos,
    /// First position after the range.
    pub end: Pos,
}

/// The compiler phase that produced a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Lexing and parsing.
    Parse,
    /// Name resolution.
    Resolve,
    /// Type checking and exhaustiveness.
    Types,
    /// Refinement / contract verification.
    Verify,
    /// Resource limits enforced at run time.
    Runtime,
}

/// Stable diagnostic codes. Names are part of the public contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Code {
    /// A token that cannot appear at this point.
    #[serde(rename = "E_PARSE_UNEXPECTED_TOKEN")]
    ParseUnexpectedToken,
    /// Expected and found types differ.
    #[serde(rename = "E_TYPE_MISMATCH")]
    TypeMismatch,
    /// A `match` does not cover every case.
    #[serde(rename = "E_MATCH_NON_EXHAUSTIVE")]
    MatchNonExhaustive,
    /// A refinement, `requires` or `ensures` can be violated.
    #[serde(rename = "E_REFINEMENT")]
    Refinement,
    /// The solver could neither prove nor refute an obligation. Rejected (soundness).
    #[serde(rename = "E_UNPROVEN")]
    Unproven,
    /// A `decreases` measure is not proven to shrink.
    #[serde(rename = "E_TERMINATION")]
    Termination,
    /// A run-time resource limit (memory, fuel) was hit.
    #[serde(rename = "E_RESOURCE")]
    Resource,
}

/// A single text edit: replace `span` with `replace` (an empty span is an insertion).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edit {
    /// Range to replace.
    pub span: Span,
    /// Replacement text.
    pub replace: String,
}

/// A machine-applicable fix made of one or more edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fix {
    /// Short human-readable description.
    pub title: String,
    /// Edits to apply together.
    pub edits: Vec<Edit>,
}

/// One problem found in the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable machine-readable code.
    pub code: Code,
    /// Phase that reported it.
    pub phase: Phase,
    /// Short, templated message.
    pub message: String,
    /// Where the problem is.
    pub span: Span,
    /// What the compiler needed to see, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
    /// Concrete values that violate the obligation, if the solver found any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterexample: Option<BTreeMap<String, String>>,
    /// Suggested fixes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fixes: Vec<Fix>,
}

/// Overall outcome of a compiler run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Compiled and verified; no diagnostics.
    Ok,
    /// At least one diagnostic.
    Error,
}

/// The complete JSON document produced by `tez check --json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Schema version, see [`SCHEMA_VERSION`].
    pub version: u32,
    /// Overall outcome.
    pub status: Status,
    /// All diagnostics found (parsers recover, so this may hold many).
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Builds a report; `status` is derived so it can never disagree with `diagnostics`.
    #[must_use]
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let status = if diagnostics.is_empty() {
            Status::Ok
        } else {
            Status::Error
        };
        Self {
            version: SCHEMA_VERSION,
            status,
            diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(line: u32, col: u32) -> Pos {
        Pos { line, col }
    }

    fn sample() -> Diagnostic {
        Diagnostic {
            code: Code::Refinement,
            phase: Phase::Verify,
            message: "divisor may be zero".into(),
            span: Span {
                start: pos(7, 14),
                end: pos(7, 19),
            },
            expected: Some("len(xs) != 0".into()),
            counterexample: Some(BTreeMap::from([("xs".to_string(), "[]".to_string())])),
            fixes: vec![Fix {
                title: "add precondition".into(),
                edits: vec![Edit {
                    span: Span {
                        start: pos(6, 33),
                        end: pos(6, 33),
                    },
                    replace: "\n  requires len(xs) > 0".into(),
                }],
            }],
        }
    }

    #[test]
    fn json_shape_is_stable() {
        let report = Report::new(vec![sample()]);
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["status"], "error");
        assert_eq!(json["diagnostics"][0]["code"], "E_REFINEMENT");
        assert_eq!(json["diagnostics"][0]["phase"], "verify");
        assert_eq!(json["diagnostics"][0]["counterexample"]["xs"], "[]");
    }

    #[test]
    fn optional_fields_are_omitted_when_empty() {
        let mut d = sample();
        d.expected = None;
        d.counterexample = None;
        d.fixes.clear();
        let json = serde_json::to_value(&d).unwrap();
        let obj = json.as_object().unwrap();
        assert!(!obj.contains_key("expected"));
        assert!(!obj.contains_key("counterexample"));
        assert!(!obj.contains_key("fixes"));
    }

    #[test]
    fn status_follows_diagnostics() {
        assert_eq!(Report::new(vec![]).status, Status::Ok);
        assert_eq!(Report::new(vec![sample()]).status, Status::Error);
    }

    #[test]
    fn round_trips() {
        let report = Report::new(vec![sample()]);
        let text = serde_json::to_string(&report).unwrap();
        let back: Report = serde_json::from_str(&text).unwrap();
        assert_eq!(report, back);
    }
}
