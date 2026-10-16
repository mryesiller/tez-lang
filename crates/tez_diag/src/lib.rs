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

macro_rules! codes {
    ($($variant:ident => $name:literal, $phase:ident;)*) => {
        /// Stable diagnostic codes. Names are part of the public contract and are
        /// documented in `spec/errors.md`; a test keeps the two in sync.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[non_exhaustive]
        #[allow(missing_docs)]
        pub enum Code {
            $(#[serde(rename = $name)] $variant,)*
        }

        impl Code {
            /// Every code, in catalog order.
            pub const ALL: &'static [Code] = &[$(Code::$variant,)*];

            /// The stable string form, e.g. `E_TYPE_MISMATCH`.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self { $(Code::$variant => $name,)* }
            }

            /// The phase that normally reports this code.
            #[must_use]
            pub const fn phase(self) -> Phase {
                match self { $(Code::$variant => Phase::$phase,)* }
            }
        }
    };
}

codes! {
    LexUnexpectedChar => "E_LEX_UNEXPECTED_CHAR", Parse;
    LexUnterminatedString => "E_LEX_UNTERMINATED_STRING", Parse;
    LexBadEscape => "E_LEX_BAD_ESCAPE", Parse;
    LexBadNumber => "E_LEX_BAD_NUMBER", Parse;
    ParseUnexpectedToken => "E_PARSE_UNEXPECTED_TOKEN", Parse;
    ParseUnexpectedEof => "E_PARSE_UNEXPECTED_EOF", Parse;
    ParseUnclosedDelimiter => "E_PARSE_UNCLOSED_DELIMITER", Parse;
    ParseExpectedExpr => "E_PARSE_EXPECTED_EXPR", Parse;
    ParseExpectedType => "E_PARSE_EXPECTED_TYPE", Parse;
    ParseExpectedPattern => "E_PARSE_EXPECTED_PATTERN", Parse;
    ParseMixedLogic => "E_PARSE_MIXED_LOGIC", Parse;
    ParseChainedComparison => "E_PARSE_CHAINED_COMPARISON", Parse;
    ParseBlockNoTail => "E_PARSE_BLOCK_NO_TAIL", Parse;
    ParseElseRequired => "E_PARSE_ELSE_REQUIRED", Parse;
    ParseStraySeparator => "E_PARSE_STRAY_SEPARATOR", Parse;
    ParseReservedWord => "E_PARSE_RESERVED_WORD", Parse;
    ParseNameCase => "E_PARSE_NAME_CASE", Parse;
    ParseDuplicateClause => "E_PARSE_DUPLICATE_CLAUSE", Parse;
    ParseBadItem => "E_PARSE_BAD_ITEM", Parse;
    NameUnknown => "E_NAME_UNKNOWN", Resolve;
    NameUnknownType => "E_NAME_UNKNOWN_TYPE", Resolve;
    NameUnknownCtor => "E_NAME_UNKNOWN_CTOR", Resolve;
    NameUnknownMethod => "E_NAME_UNKNOWN_METHOD", Resolve;
    NameDuplicate => "E_NAME_DUPLICATE", Resolve;
    NameShadowing => "E_NAME_SHADOWING", Resolve;
    NamePrelude => "E_NAME_PRELUDE", Resolve;
    NameUseMethodSyntax => "E_NAME_USE_METHOD_SYNTAX", Resolve;
    ContractScope => "E_CONTRACT_SCOPE", Resolve;
    ConstInit => "E_CONST_INIT", Resolve;
    UseNotFound => "E_USE_NOT_FOUND", Resolve;
    UseCycle => "E_USE_CYCLE", Resolve;
    UseBadPath => "E_USE_BAD_PATH", Resolve;
    UseMissing => "E_USE_MISSING", Resolve;
    TypeMismatch => "E_TYPE_MISMATCH", Types;
    TypeArity => "E_TYPE_ARITY", Types;
    TypeNotFunction => "E_TYPE_NOT_FUNCTION", Types;
    TypeUnknownField => "E_TYPE_UNKNOWN_FIELD", Types;
    TypeMissingField => "E_TYPE_MISSING_FIELD", Types;
    TypeExtraField => "E_TYPE_EXTRA_FIELD", Types;
    TypeAmbiguous => "E_TYPE_AMBIGUOUS", Types;
    TypeInfinite => "E_TYPE_INFINITE", Types;
    TypeNotBool => "E_TYPE_NOT_BOOL", Types;
    TypeBadOperand => "E_TYPE_BAD_OPERAND", Types;
    TypeNotEquatable => "E_TYPE_NOT_EQUATABLE", Types;
    TypeNotOrdered => "E_TYPE_NOT_ORDERED", Types;
    TypeTupleIndex => "E_TYPE_TUPLE_INDEX", Types;
    TypeRefinementPosition => "E_TYPE_REFINEMENT_POSITION", Types;
    TypeRecursiveAlias => "E_TYPE_RECURSIVE_ALIAS", Types;
    MatchNonExhaustive => "E_MATCH_NON_EXHAUSTIVE", Types;
    MatchUnreachable => "E_MATCH_UNREACHABLE", Types;
    MatchBindingMismatch => "E_MATCH_BINDING_MISMATCH", Types;
    DivZero => "E_DIV_ZERO", Verify;
    FloatDomain => "E_FLOAT_DOMAIN", Verify;
    IndexBounds => "E_INDEX_BOUNDS", Verify;
    Precondition => "E_PRECONDITION", Verify;
    Postcondition => "E_POSTCONDITION", Verify;
    Refinement => "E_REFINEMENT", Verify;
    Assert => "E_ASSERT", Verify;
    TerminationMissing => "E_TERMINATION_MISSING", Verify;
    Termination => "E_TERMINATION", Verify;
    Unproven => "E_UNPROVEN", Verify;
    Resource => "E_RESOURCE", Runtime;
    TestFailed => "E_TEST_FAILED", Runtime;
    Internal => "E_INTERNAL", Runtime;
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
    /// The project file the span is in, when the program has more than one file (relative path, `/` separators).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
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

impl Diagnostic {
    /// A diagnostic whose phase is the code's usual phase.
    #[must_use]
    pub fn new(code: Code, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            phase: code.phase(),
            message: message.into(),
            span,
            file: None,
            expected: None,
            counterexample: None,
            fixes: Vec::new(),
        }
    }

    /// Sets the `expected` field.
    #[must_use]
    pub fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Adds a one-edit fix that replaces `span` with `replace`.
    #[must_use]
    pub fn fix(mut self, title: impl Into<String>, span: Span, replace: impl Into<String>) -> Self {
        self.fixes.push(Fix {
            title: title.into(),
            edits: vec![Edit {
                span,
                replace: replace.into(),
            }],
        });
        self
    }

    /// Moves the diagnostic to another phase (e.g. verification codes found at run time).
    #[must_use]
    pub fn in_phase(mut self, phase: Phase) -> Self {
        self.phase = phase;
        self
    }
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
            file: None,
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
    fn codes_are_unique_and_prefixed() {
        let mut seen = std::collections::BTreeSet::new();
        for code in Code::ALL {
            assert!(code.as_str().starts_with("E_"), "{}", code.as_str());
            assert!(seen.insert(code.as_str()), "duplicate {}", code.as_str());
        }
    }

    #[test]
    fn every_code_is_documented_in_the_catalog() {
        let catalog = include_str!("../../../spec/errors.md");
        for code in Code::ALL {
            assert!(
                catalog.contains(&format!("`{}`", code.as_str())),
                "{} missing from spec/errors.md",
                code.as_str()
            );
        }
    }

    #[test]
    fn every_catalog_code_exists() {
        let catalog = include_str!("../../../spec/errors.md");
        for line in catalog.lines().filter(|l| l.starts_with("| `E_")) {
            let name = line.trim_start_matches("| `").split('`').next().unwrap();
            assert!(
                Code::ALL.iter().any(|c| c.as_str() == name),
                "{name} in catalog but not in Code"
            );
        }
    }

    #[test]
    fn round_trips() {
        let report = Report::new(vec![sample()]);
        let text = serde_json::to_string(&report).unwrap();
        let back: Report = serde_json::from_str(&text).unwrap();
        assert_eq!(report, back);
    }
}
