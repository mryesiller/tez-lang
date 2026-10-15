//! Source text in, a verified program and a diagnostics report out.
//!
//! One function ([`compile`]) runs every phase in order and stops at the first phase that
//! reports problems, so later phases never see ill-formed input.

use tez_diag::{Diagnostic, Report};
use tez_syntax::{ByteSpan, LineIndex, ParseOptions, Problem, parse};
use tez_types::hir::Program;

/// Options for [`compile`].
#[derive(Debug, Clone)]
pub struct CompileOptions {
    /// Prove contracts, refinements and termination (needs the `z3` solver). On by default.
    pub verify: bool,
    /// Which solver backend proves obligations (native process, or script replay for the browser).
    pub backend: tez_verify::Backend,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            verify: true,
            backend: tez_verify::Backend::default(),
        }
    }
}

/// One source file of a program.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Path relative to the project directory, `/`-separated (the entry file is just its name).
    pub name: String,
    /// The text as read.
    pub text: String,
}

/// Result of compiling a program (one file, or an entry file and the files it `use`s).
#[derive(Debug)]
pub struct Compiled {
    /// Everything wrong with the source; empty means the program is accepted.
    pub report: Report,
    /// The checked program, present only when the report is clean.
    pub program: Option<Program>,
    /// Every file that was read, entry file last in dependency order. Diagnostics name these.
    pub files: Vec<SourceFile>,
    /// The program text the later phases saw: all files merged (spans of the program refer to it).
    pub merged: String,
    /// Where each file starts and ends in `merged`.
    pub ranges: Vec<(usize, usize)>,
}

impl Compiled {
    /// Maps an offset in the merged text to `(index into files, offset inside that file)`.
    #[must_use]
    pub fn locate(&self, offset: usize) -> (usize, usize) {
        let k = self
            .ranges
            .iter()
            .position(|&(s, e)| offset >= s && offset < e.max(s + 1))
            .unwrap_or(self.files.len().saturating_sub(1));
        let start = self.ranges.get(k).map_or(0, |r| r.0);
        (k, offset.saturating_sub(start))
    }
}

/// Where the files of a project come from: a relative path in, the text out (`None` = no such file).
pub type Loader<'a> = &'a dyn Fn(&str) -> Option<String>;

fn diagnostics(src: &str, problems: &[Problem]) -> Vec<Diagnostic> {
    let index = LineIndex::new(src);
    let mut ps: Vec<&Problem> = problems.iter().collect();
    ps.sort_by_key(|p| p.span.start);
    ps.into_iter().map(|p| p.to_diagnostic(&index)).collect()
}

/// Runs parse, name resolution, type checking and (by default) verification on one source text.
/// `use` lines cannot be followed here (there are no other files): they are reported as missing.
#[must_use]
pub fn compile(src: &str, options: &CompileOptions) -> Compiled {
    compile_project("main.tez", src, &|_| None, options)
}

fn shifted(p: &Problem, delta: usize, limit: usize) -> Problem {
    let mut q = p.clone();
    let fix = |sp: ByteSpan| {
        ByteSpan::new(
            sp.start.saturating_sub(delta).min(limit),
            sp.end.saturating_sub(delta).min(limit),
        )
    };
    q.span = fix(q.span);
    for f in &mut q.fixes {
        f.span = fix(f.span);
    }
    q
}

/// Turns problems with spans in the merged text into diagnostics positioned in their own files.
fn project_diagnostics(
    merged: &[(usize, usize)],
    files: &[SourceFile],
    entry: usize,
    problems: &[Problem],
    multi: bool,
) -> Vec<Diagnostic> {
    let mut ps: Vec<&Problem> = problems.iter().collect();
    ps.sort_by_key(|p| p.span.start);
    ps.into_iter()
        .map(|p| {
            let k = merged
                .iter()
                .position(|&(s, e)| p.span.start >= s && p.span.start < e.max(s + 1))
                .unwrap_or(entry);
            let (s, e) = merged[k];
            let local = shifted(p, s, e - s);
            let mut d = local.to_diagnostic(&LineIndex::new(&files[k].text));
            if multi {
                d.file = Some(files[k].name.clone());
            }
            d
        })
        .collect()
}

/// `dir/of/importer` + `"rel/path.tez"` → a normalized project path, or why it is not allowed.
fn resolve_path(importer: &str, path: &str) -> Result<String, String> {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return Err("an import path must be relative, e.g. `use \"geometry.tez\"`".into());
    }
    if !std::path::Path::new(path)
        .extension()
        .is_some_and(|e| e == "tez")
    {
        return Err("an import path must end in `.tez`".into());
    }
    let mut parts: Vec<&str> = importer.split('/').collect();
    parts.pop(); // the importing file's own name
    for c in path.split('/') {
        match c {
            "" => return Err("an import path must not contain `//`".into()),
            "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err("an import path must stay inside the project directory".into());
                }
            }
            other => parts.push(other),
        }
    }
    Ok(parts.join("/"))
}

struct Project<'a> {
    load: Loader<'a>,
    /// Files in dependency order (dependencies first), as discovered.
    order: Vec<SourceFile>,
    /// Spans of the `use` lines of each file (blanked in the merged text).
    use_spans: Vec<Vec<ByteSpan>>,
    done: Vec<String>,
    visiting: Vec<String>,
    problems: Vec<(String, Problem)>,
}

impl Project<'_> {
    fn visit(&mut self, name: &str, text: String) {
        self.visiting.push(name.to_string());
        let parsed = parse(&text, ParseOptions::default());
        let mut uses = Vec::new();
        if !parsed.problems.is_empty() {
            for p in parsed.problems {
                self.problems.push((name.to_string(), p));
            }
        } else {
            for item in &parsed.file.items {
                let tez_syntax::ast::Item::Use(u) = item else {
                    continue;
                };
                uses.push(u.span);
                match resolve_path(name, &u.path) {
                    Err(msg) => self.problems.push((
                        name.to_string(),
                        Problem::new(tez_diag::Code::UseBadPath, msg, u.span),
                    )),
                    Ok(target) => {
                        if self.visiting.contains(&target) {
                            let chain = self
                                .visiting
                                .iter()
                                .skip_while(|f| **f != target)
                                .chain(std::iter::once(&target))
                                .cloned()
                                .collect::<Vec<_>>()
                                .join(" -> ");
                            self.problems.push((
                                name.to_string(),
                                Problem::new(
                                    tez_diag::Code::UseCycle,
                                    format!("these files import each other: {chain}"),
                                    u.span,
                                )
                                .expected("an import graph without cycles"),
                            ));
                        } else if !self.done.contains(&target) {
                            match (self.load)(&target) {
                                None => self.problems.push((
                                    name.to_string(),
                                    Problem::new(
                                        tez_diag::Code::UseNotFound,
                                        format!("cannot find `{target}`"),
                                        u.span,
                                    )
                                    .expected("an existing file, relative to this one"),
                                )),
                                Some(t) => self.visit(&target, t),
                            }
                        }
                    }
                }
            }
        }
        self.visiting.pop();
        self.done.push(name.to_string());
        self.order.push(SourceFile {
            name: name.to_string(),
            text,
        });
        self.use_spans.push(uses);
    }
}

/// Compiles a program made of `entry_name` (whose text is `entry_src`) and the files it `use`s, transitively.
/// Each file is read once through `load`; names across all files share one namespace (no shadowing, as
/// everywhere in Tez). Diagnostics carry the file they are in.
#[must_use]
pub fn compile_project(
    entry_name: &str,
    entry_src: &str,
    load: Loader<'_>,
    options: &CompileOptions,
) -> Compiled {
    let mut project = Project {
        load,
        order: Vec::new(),
        use_spans: Vec::new(),
        done: Vec::new(),
        visiting: Vec::new(),
        problems: Vec::new(),
    };
    project.visit(entry_name, entry_src.to_string());
    let files = std::mem::take(&mut project.order);
    let multi = files.len() > 1;
    let entry = files.len() - 1;

    if !project.problems.is_empty() {
        let mut diags = Vec::new();
        for f in &files {
            let mine: Vec<Problem> = project
                .problems
                .iter()
                .filter(|(n, _)| *n == f.name)
                .map(|(_, p)| p.clone())
                .collect();
            for mut d in diagnostics(&f.text, &mine) {
                if multi {
                    d.file = Some(f.name.clone());
                }
                diags.push(d);
            }
        }
        return Compiled {
            report: Report::new(diags),
            program: None,
            files,
            merged: String::new(),
            ranges: Vec::new(),
        };
    }

    // merge: dependencies first, `use` lines blanked (same length, so every other offset is unchanged)
    let mut merged_text = String::new();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for (f, spans) in files.iter().zip(&project.use_spans) {
        let start = merged_text.len();
        let mut t = f.text.clone().into_bytes();
        for sp in spans {
            for b in &mut t[sp.start..sp.end] {
                if *b != b'\n' {
                    *b = b' ';
                }
            }
        }
        merged_text.push_str(&String::from_utf8_lossy(&t));
        ranges.push((start, merged_text.len()));
        merged_text.push('\n');
    }
    let src = merged_text.clone();
    let src = src.as_str();

    let parsed = parse(src, ParseOptions::default());
    if !parsed.problems.is_empty() {
        return Compiled {
            report: Report::new(project_diagnostics(
                &ranges,
                &files,
                entry,
                &parsed.problems,
                multi,
            )),
            program: None,
            files,
            merged: merged_text.clone(),
            ranges: ranges.clone(),
        };
    }
    let checked = tez_types::check(&parsed.file, src);
    if !checked.problems.is_empty() {
        return Compiled {
            report: Report::new(project_diagnostics(
                &ranges,
                &files,
                entry,
                &checked.problems,
                multi,
            )),
            program: None,
            files,
            merged: merged_text.clone(),
            ranges: ranges.clone(),
        };
    }
    if options.verify {
        let verified = tez_verify::verify(
            &checked.program,
            src,
            &tez_verify::VerifyOptions {
                backend: options.backend,
                ..tez_verify::VerifyOptions::default()
            },
        );
        let problems = match verified {
            Ok(p) => p,
            Err(msg) => vec![
                Problem::new(
                    tez_diag::Code::Internal,
                    format!("the verifier could not run: {msg}"),
                    ByteSpan::new(0, 0),
                )
                .expected("the `z3` SMT solver on PATH (nothing is accepted without verification)"),
            ],
        };
        if !problems.is_empty() {
            return Compiled {
                report: Report::new(project_diagnostics(
                    &ranges, &files, entry, &problems, multi,
                )),
                program: None,
                files,
                merged: merged_text.clone(),
                ranges: ranges.clone(),
            };
        }
    }
    Compiled {
        report: Report::new(Vec::new()),
        program: Some(checked.program),
        files,
        merged: merged_text.clone(),
        ranges,
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

/// Like [`render_human`] for a program of several files: each diagnostic is shown with the text and path of the
/// file it is in. `dir` is prepended to project-relative names (the entry file is shown as `entry_path`).
#[must_use]
pub fn render_human_project(
    report: &Report,
    files: &[SourceFile],
    dir: &str,
    entry_path: &str,
) -> String {
    let entry = files.last().map(|f| f.name.as_str());
    let mut out = String::new();
    for d in &report.diagnostics {
        let name = d.file.as_deref().or(entry).unwrap_or("");
        let text = files
            .iter()
            .find(|f| f.name == name)
            .map_or("", |f| f.text.as_str());
        let shown = if Some(name) == entry {
            entry_path.to_string()
        } else {
            format!("{dir}{name}")
        };
        let one = Report::new(vec![Diagnostic {
            file: None,
            ..d.clone()
        }]);
        out.push_str(&render_human(&one, text, &shown));
    }
    out
}
