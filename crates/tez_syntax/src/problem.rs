use tez_diag::{Code, Diagnostic};

use crate::source::{ByteSpan, LineIndex};

/// A machine-applicable edit attached to a [`Problem`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemFix {
    /// Short description.
    pub title: String,
    /// Range to replace (empty = insertion).
    pub span: ByteSpan,
    /// Replacement text.
    pub replace: String,
}

/// A diagnostic in byte-offset form. Converted to a line/column [`Diagnostic`]
/// once the source text is at hand ([`Problem::to_diagnostic`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Stable code.
    pub code: Code,
    /// Short templated message.
    pub message: String,
    /// Offending range.
    pub span: ByteSpan,
    /// What the compiler needed, in Tez syntax.
    pub expected: Option<String>,
    /// Suggested fixes.
    pub fixes: Vec<ProblemFix>,
}

impl Problem {
    /// Creates a problem with no `expected` text and no fixes.
    #[must_use]
    pub fn new(code: Code, message: impl Into<String>, span: ByteSpan) -> Self {
        Self {
            code,
            message: message.into(),
            span,
            expected: None,
            fixes: Vec::new(),
        }
    }

    /// Sets the `expected` text.
    #[must_use]
    pub fn expected(mut self, expected: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self
    }

    /// Adds a fix replacing `span` with `replace`.
    #[must_use]
    pub fn fix(
        mut self,
        title: impl Into<String>,
        span: ByteSpan,
        replace: impl Into<String>,
    ) -> Self {
        self.fixes.push(ProblemFix {
            title: title.into(),
            span,
            replace: replace.into(),
        });
        self
    }

    /// Converts to the JSON-facing diagnostic using `index` for positions.
    #[must_use]
    pub fn to_diagnostic(&self, index: &LineIndex<'_>) -> Diagnostic {
        let mut d = Diagnostic::new(self.code, self.message.clone(), index.span(self.span));
        d.expected = self.expected.clone();
        for f in &self.fixes {
            d = d.fix(f.title.clone(), index.span(f.span), f.replace.clone());
        }
        d
    }
}
