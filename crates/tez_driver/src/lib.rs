//! Source text in, a verified program and a diagnostics report out.
//!
//! One function ([`compile`]) runs every phase in order and stops at the first phase that
//! reports problems, so later phases never see ill-formed input.

use tez_diag::{Diagnostic, Report};
use tez_syntax::{LineIndex, ParseOptions, Problem, parse};
use tez_types::hir::Program;

/// Options for [`compile`].
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {}

/// Result of compiling one source file.
#[derive(Debug)]
pub struct Compiled {
    /// Everything wrong with the source; empty means the program is accepted.
    pub report: Report,
    /// The checked program, present only when the report is clean.
    pub program: Option<Program>,
}

fn diagnostics(src: &str, problems: &[Problem]) -> Vec<Diagnostic> {
    let index = LineIndex::new(src);
    let mut ps: Vec<&Problem> = problems.iter().collect();
    ps.sort_by_key(|p| p.span.start);
    ps.into_iter().map(|p| p.to_diagnostic(&index)).collect()
}

/// Runs parse, name resolution and type checking.
#[must_use]
pub fn compile(src: &str, _options: &CompileOptions) -> Compiled {
    let parsed = parse(src, ParseOptions::default());
    if !parsed.problems.is_empty() {
        return Compiled {
            report: Report::new(diagnostics(src, &parsed.problems)),
            program: None,
        };
    }
    let checked = tez_types::check(&parsed.file, src);
    if !checked.problems.is_empty() {
        return Compiled {
            report: Report::new(diagnostics(src, &checked.problems)),
            program: None,
        };
    }
    Compiled {
        report: Report::new(Vec::new()),
        program: Some(checked.program),
    }
}

/// Renders diagnostics for a terminal, with source excerpts and carets.
#[must_use]
pub fn render_human(report: &Report, src: &str, path: &str) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut out = String::new();
    for d in &report.diagnostics {
        out.push_str(&format!("error[{}]: {}\n", d.code.as_str(), d.message));
        out.push_str(&format!(
            "  --> {path}:{}:{}\n",
            d.span.start.line, d.span.start.col
        ));
        let ln = d.span.start.line as usize;
        if let Some(line) = lines.get(ln.saturating_sub(1)) {
            let gutter = ln.to_string();
            let pad = " ".repeat(gutter.len());
            let start = d.span.start.col.saturating_sub(1) as usize;
            let width = if d.span.end.line == d.span.start.line {
                (d.span.end.col.saturating_sub(d.span.start.col)).max(1) as usize
            } else {
                line.chars().count().saturating_sub(start).max(1)
            };
            out.push_str(&format!(
                "{pad} |\n{gutter} | {line}\n{pad} | {}{}\n",
                " ".repeat(start),
                "^".repeat(width)
            ));
        }
        if let Some(e) = &d.expected {
            out.push_str(&format!("   = expected: {e}\n"));
        }
        if let Some(c) = &d.counterexample {
            let parts: Vec<String> = c.iter().map(|(k, v)| format!("{k} = {v}")).collect();
            out.push_str(&format!("   = counterexample: {}\n", parts.join(", ")));
        }
        for f in &d.fixes {
            let e = f.edits.first().map_or(String::new(), |e| {
                format!(" -> `{}`", e.replace.replace('\n', "\\n"))
            });
            out.push_str(&format!("   = fix: {}{e}\n", f.title));
        }
        out.push('\n');
    }
    out
}
