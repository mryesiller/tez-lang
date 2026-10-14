//! A small Wadler-style document model and renderer.

/// A layout document.
#[derive(Debug, Clone)]
pub enum Doc {
    /// Nothing.
    Nil,
    /// Literal text without newlines.
    Text(String),
    /// Sequence.
    Concat(Vec<Doc>),
    /// Increases indentation of line breaks inside.
    Indent(Box<Doc>),
    /// Laid out flat if it fits on the line, otherwise its direct line breaks break.
    Group(Box<Doc>),
    /// A space when flat, a newline when broken.
    Line,
    /// Nothing when flat, a newline when broken.
    SoftLine,
    /// Always a newline (forces enclosing groups to break).
    HardLine,
    /// Text printed at the end of the current line (zero width), for trailing comments.
    Suffix(String),
}

/// Text.
#[must_use]
pub fn text(s: impl Into<String>) -> Doc {
    Doc::Text(s.into())
}

/// Sequence.
#[must_use]
pub fn concat(ds: Vec<Doc>) -> Doc {
    Doc::Concat(ds)
}

/// Group.
#[must_use]
pub fn group(d: Doc) -> Doc {
    Doc::Group(Box::new(d))
}

/// Indent.
#[must_use]
pub fn indent(d: Doc) -> Doc {
    Doc::Indent(Box::new(d))
}

/// Joins documents with a separator.
#[must_use]
pub fn join(ds: Vec<Doc>, sep: &Doc) -> Doc {
    let mut out = Vec::new();
    for (i, d) in ds.into_iter().enumerate() {
        if i > 0 {
            out.push(sep.clone());
        }
        out.push(d);
    }
    Doc::Concat(out)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

fn width_of(s: &str) -> usize {
    s.chars().count()
}

fn fits(mut remaining: isize, first: (usize, Mode, &Doc), rest: &[(usize, Mode, &Doc)]) -> bool {
    let mut stack: Vec<(usize, Mode, &Doc)> = vec![first];
    let mut rest_iter = rest.iter().rev();
    loop {
        if remaining < 0 {
            return false;
        }
        let Some((ind, mode, d)) = stack.pop().or_else(|| rest_iter.next().copied()) else {
            return true;
        };
        match d {
            Doc::Nil | Doc::Suffix(_) => {}
            Doc::Text(s) => remaining -= isize::try_from(width_of(s)).unwrap_or(isize::MAX),
            Doc::Concat(ds) => stack.extend(ds.iter().rev().map(|x| (ind, mode, x))),
            Doc::Indent(x) => stack.push((ind + 2, mode, x)),
            Doc::Group(x) => stack.push((ind, mode, x)),
            Doc::Line => match mode {
                Mode::Flat => remaining -= 1,
                Mode::Break => return true,
            },
            Doc::SoftLine => {
                if mode == Mode::Break {
                    return true;
                }
            }
            Doc::HardLine => return mode == Mode::Break,
        }
    }
}

/// Renders `doc` with lines at most `width` columns where possible.
#[must_use]
pub fn render(doc: &Doc, width: usize) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    let mut suffix = String::new();
    let mut stack: Vec<(usize, Mode, &Doc)> = vec![(0, Mode::Break, doc)];
    let newline = |out: &mut String, col: &mut usize, suffix: &mut String, ind: usize| {
        out.push_str(suffix);
        suffix.clear();
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
        out.push_str(&" ".repeat(ind));
        *col = ind;
    };
    while let Some((ind, mode, d)) = stack.pop() {
        match d {
            Doc::Nil => {}
            Doc::Text(s) => {
                out.push_str(s);
                col += width_of(s);
            }
            Doc::Concat(ds) => stack.extend(ds.iter().rev().map(|x| (ind, mode, x))),
            Doc::Indent(x) => stack.push((ind + 2, mode, x)),
            Doc::Group(x) => {
                let flat = mode == Mode::Flat
                    || fits(
                        isize::try_from(width.saturating_sub(col)).unwrap_or(isize::MAX),
                        (ind, Mode::Flat, x),
                        &stack,
                    );
                stack.push((ind, if flat { Mode::Flat } else { Mode::Break }, x));
            }
            Doc::Line => match mode {
                Mode::Flat => {
                    out.push(' ');
                    col += 1;
                }
                Mode::Break => newline(&mut out, &mut col, &mut suffix, ind),
            },
            Doc::SoftLine => {
                if mode == Mode::Break {
                    newline(&mut out, &mut col, &mut suffix, ind);
                }
            }
            Doc::HardLine => newline(&mut out, &mut col, &mut suffix, ind),
            Doc::Suffix(s) => suffix.push_str(s),
        }
    }
    out.push_str(&suffix);
    while out.ends_with(' ') {
        out.pop();
    }
    out
}
