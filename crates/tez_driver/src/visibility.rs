//! Which names a file of a project may use: its own, and those of the files it `use`s directly.
//!
//! All files of a program share one namespace (names are unique across the project), so this is not name
//! resolution: it runs after type checking and only rejects. A file that relies on a name it did not import would
//! stop compiling the moment the file that happened to bring that name in is removed; requiring the `use` keeps
//! every file honest about what it depends on.
//!
//! Capitalised names (types, constructors, `Type.method` qualifiers) are found in the tokens of each file: since
//! no name is shadowed, such a token always means the declaration of that name. Lower-case names (functions,
//! constants, methods called with `.m()`) are found in the checked program, which knows what each call resolved to.

use std::collections::{HashMap, HashSet};

use tez_diag::Code;
use tez_syntax::ast::{File, Item, TypeBody};
use tez_syntax::{ByteSpan, Problem, Tok, lex};
use tez_types::hir::{Expr, ExprKind, Program, TypeKind};

/// The files of a merged program, as the visibility check sees them.
pub(crate) struct Files<'a> {
    /// Project-relative names, in merge order.
    pub names: &'a [String],
    /// Where each file lies in the merged text.
    pub ranges: &'a [(usize, usize)],
    /// For each file, the indices of the files it `use`s.
    pub uses: &'a [Vec<usize>],
}

impl Files<'_> {
    fn of(&self, offset: usize) -> Option<usize> {
        self.ranges
            .iter()
            .position(|&(s, e)| offset >= s && offset < e.max(s + 1))
    }

    fn sees(&self, site: usize, def: usize) -> bool {
        site == def || self.uses.get(site).is_some_and(|u| u.contains(&def))
    }

    /// Whether `from` imports `to`, directly or through other files.
    fn reaches(&self, from: usize, to: usize) -> bool {
        let mut stack = vec![from];
        let mut seen = HashSet::new();
        while let Some(k) = stack.pop() {
            if k == to {
                return true;
            }
            if seen.insert(k) {
                stack.extend(self.uses.get(k).into_iter().flatten());
            }
        }
        false
    }
}

/// `to`, written relative to the directory of `from` (both project-relative, `/`-separated).
fn relative(from: &str, to: &str) -> String {
    let mut from_dir: Vec<&str> = from.split('/').collect();
    from_dir.pop();
    let to_parts: Vec<&str> = to.split('/').collect();
    let common = from_dir
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut out: Vec<&str> = vec![".."; from_dir.len() - common];
    out.extend(&to_parts[common..]);
    out.join("/")
}

/// Problems for every place where a file uses a name declared in a file it does not `use`.
/// `merged` is the program text, `file` its parse and `program` its checked form.
pub(crate) fn check(
    files: &Files<'_>,
    merged: &str,
    file: &File,
    program: &Program,
) -> Vec<Problem> {
    // where each user-declared capitalised name lives (types and their constructors)
    let mut upper: HashMap<&str, Vec<usize>> = HashMap::new();
    for item in &file.items {
        let Item::Type(td) = item else { continue };
        let Some(k) = files.of(td.span.start) else {
            continue;
        };
        upper.entry(td.name.name.as_str()).or_default().push(k);
        if let TypeBody::Adt(variants) = &td.body {
            for v in variants {
                upper.entry(v.name.name.as_str()).or_default().push(k);
            }
        }
    }

    let mut missing: Vec<(usize, usize, String, ByteSpan)> = Vec::new();
    let mut note = |site: usize, def: usize, name: &str, span: ByteSpan| {
        if !files.sees(site, def) {
            missing.push((site, def, name.to_string(), span));
        }
    };

    for (site, &(start, end)) in files.ranges.iter().enumerate() {
        let Some(text) = merged.get(start..end) else {
            continue;
        };
        for t in lex(text).tokens {
            let Tok::Upper(name) = &t.tok else { continue };
            let Some(defs) = upper.get(name.as_str()) else {
                continue;
            };
            if !defs.iter().any(|&d| files.sees(site, d)) {
                let span = ByteSpan::new(t.span.start + start, t.span.end + start);
                if let Some(&d) = defs.first() {
                    note(site, d, name, span);
                }
            }
        }
    }

    let fn_file: Vec<Option<usize>> = program
        .fns
        .iter()
        .map(|f| {
            if f.is_prelude {
                None
            } else {
                files.of(f.span.start)
            }
        })
        .collect();
    let const_file: Vec<Option<usize>> = program
        .consts
        .iter()
        .map(|c| files.of(c.span.start))
        .collect();
    let mut visit = |e: &Expr| {
        let (def, name) = match &e.kind {
            ExprKind::Call { f, .. } | ExprKind::FnRef(f, _) => (
                fn_file.get(*f).copied().flatten(),
                program.fns.get(*f).map(|d| d.full_name()),
            ),
            ExprKind::Const(c) => (
                const_file.get(*c).copied().flatten(),
                program.consts.get(*c).map(|d| d.name.clone()),
            ),
            _ => return,
        };
        if let (Some(def), Some(name), Some(site)) = (def, name, files.of(e.span.start)) {
            note(site, def, &name, e.span);
        }
    };
    for f in program.fns.iter().filter(|f| !f.is_prelude) {
        let refines = f
            .params
            .iter()
            .flat_map(|p| &p.refine)
            .chain(&f.ret_refine)
            .map(|r| &r.pred);
        for e in f
            .body
            .iter()
            .chain(&f.requires)
            .chain(&f.ensures)
            .chain(&f.decreases)
            .chain(refines)
        {
            e.walk(&mut visit);
        }
    }
    for c in &program.consts {
        c.value.walk(&mut visit);
        c.refine.iter().for_each(|r| r.pred.walk(&mut visit));
    }
    for t in &program.tests {
        t.body.walk(&mut visit);
    }
    for t in program.types.iter().filter(|t| !t.is_prelude) {
        let fields: Vec<_> = match &t.kind {
            TypeKind::Record(fs) => fs.iter().collect(),
            TypeKind::Adt(vs) => vs.iter().flat_map(|v| &v.fields).collect(),
        };
        for fd in fields {
            fd.refine.iter().for_each(|r| r.pred.walk(&mut visit));
        }
    }

    // one problem per (file, missing import), at its first use
    missing.sort_by_key(|m| m.3.start);
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    missing.retain(|m| seen.insert((m.0, m.1)));
    missing
        .into_iter()
        .map(|(site, def, name, span)| {
            let (Some(site_name), Some(def_name)) = (files.names.get(site), files.names.get(def)) else {
                return Problem::new(Code::Internal, "a file of the project is missing", span);
            };
            let path = relative(site_name, def_name);
            let at = files.ranges.get(site).map_or(0, |r| r.0);
            let p = Problem::new(
                Code::UseMissing,
                format!("`{name}` is declared in `{def_name}`, which this file does not `use`"),
                span,
            );
            if files.reaches(def, site) {
                // `def` already depends on this file: importing it back would be a cycle
                return p.expected(format!(
                    "`{name}` moved into this file or into a file that both `{site_name}` and `{def_name}` use \
                     (`{def_name}` already imports this file, so it cannot be imported back)"
                ));
            }
            p.expected(format!("use \"{path}\" at the top of this file"))
            .fix(
                format!("add `use \"{path}\"`"),
                ByteSpan::new(at, at),
                format!("use \"{path}\"\n"),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::relative;

    #[test]
    fn relative_paths() {
        assert_eq!(relative("main.tez", "util/money.tez"), "util/money.tez");
        assert_eq!(relative("cart.tez", "util/money.tez"), "util/money.tez");
        assert_eq!(relative("util/money.tez", "cart.tez"), "../cart.tez");
        assert_eq!(relative("a/b/x.tez", "a/c/y.tez"), "../c/y.tez");
        assert_eq!(relative("a/x.tez", "a/y.tez"), "y.tez");
    }
}
