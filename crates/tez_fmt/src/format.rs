use tez_syntax::ast::{
    Arm, BinOp, ConstDecl, Expr, ExprKind, FieldInit, File, FnDecl, Item, Lit, Pat, PatKind, Stmt,
    TestDecl, TypeBody, TypeDecl, TypeExpr, TypeKind, UnOp,
};
use tez_syntax::{ByteSpan, Comment, ParseOptions, Problem, parse};

use crate::doc::{Doc, concat, group, indent, join, render, text};

const WIDTH: usize = 100;

/// Why formatting did not produce output.
#[derive(Debug)]
pub enum FmtError {
    /// The source does not parse; fix these first.
    Syntax(Vec<Problem>),
    /// A comment sits where the formatter cannot keep it (inside an expression).
    Comment(ByteSpan),
}

/// Formats Tez source text.
///
/// # Errors
/// [`FmtError::Syntax`] when the source has syntax problems, [`FmtError::Comment`] when a
/// comment cannot be placed.
pub fn format_source(src: &str, options: ParseOptions) -> Result<String, FmtError> {
    let out = parse(src, options);
    if !out.problems.is_empty() {
        return Err(FmtError::Syntax(out.problems));
    }
    format_file(&out.file, &out.comments, src)
}

/// Formats an already parsed file.
///
/// # Errors
/// [`FmtError::Comment`] when a comment cannot be placed.
pub fn format_file(file: &File, comments: &[Comment], src: &str) -> Result<String, FmtError> {
    let mut f = Fmt {
        src,
        comments,
        used: vec![false; comments.len()],
        error: None,
    };
    let doc = f.file(file);
    if let Some(span) = f.error {
        return Err(FmtError::Comment(span));
    }
    if let Some(i) = f.used.iter().position(|u| !u) {
        return Err(FmtError::Comment(comments[i].span));
    }
    let mut s = render(&doc, WIDTH);
    if !s.is_empty() {
        s.push('\n');
    }
    Ok(s)
}

struct Fmt<'a> {
    src: &'a str,
    comments: &'a [Comment],
    used: Vec<bool>,
    error: Option<ByteSpan>,
}

fn hard() -> Doc {
    Doc::HardLine
}

fn escape(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl Fmt<'_> {
    // ------------------------------------------------------------ comments

    fn comment_text(&self, c: &Comment) -> String {
        c.span.text(self.src).trim_end().to_string()
    }

    /// Own-line comments that precede a node starting at `before`.
    fn leading(&mut self, before: usize) -> Vec<Doc> {
        let mut out = Vec::new();
        for (i, c) in self.comments.iter().enumerate() {
            if !self.used[i] && c.own_line && c.span.start < before {
                self.used[i] = true;
                out.push(text(self.comment_text(c)));
                out.push(hard());
            }
        }
        out
    }

    /// A same-line comment right after a node ending at `after`.
    fn trailing(&mut self, after: usize) -> Doc {
        for (i, c) in self.comments.iter().enumerate() {
            if self.used[i] || c.own_line || c.span.start < after {
                continue;
            }
            if self.src[after..c.span.start].contains('\n') {
                break;
            }
            self.used[i] = true;
            return Doc::Suffix(format!(" {}", self.comment_text(c)));
        }
        Doc::Nil
    }

    /// Reports comments stranded inside `span` (an unsupported position).
    fn check_inside(&mut self, span: ByteSpan) {
        for (i, c) in self.comments.iter().enumerate() {
            if !self.used[i]
                && c.span.start >= span.start
                && c.span.end <= span.end
                && self.error.is_none()
            {
                self.error = Some(c.span);
            }
        }
    }

    // ------------------------------------------------------------ file and items

    fn file(&mut self, file: &File) -> Doc {
        let mut parts = Vec::new();
        for (i, item) in file.items.iter().enumerate() {
            if i > 0 {
                parts.push(hard());
                parts.push(hard());
            }
            let span = item.span();
            parts.extend(self.leading(span.start));
            let d = match item {
                Item::Type(t) => self.type_decl(t),
                Item::Fn(f) => self.fn_decl(f),
                Item::Const(c) => self.const_decl(c),
                Item::Test(t) => self.test_decl(t),
            };
            parts.push(d);
            parts.push(self.trailing(span.end));
            self.check_inside(span);
        }
        let end = self.src.len();
        let tail = self.leading(end);
        if !tail.is_empty() {
            if !parts.is_empty() {
                parts.push(hard());
                parts.push(hard());
            }
            let n = tail.len();
            parts.extend(tail.into_iter().take(n.saturating_sub(1)));
        }
        concat(parts)
    }

    fn tparams(ts: &[tez_syntax::ast::Ident]) -> String {
        if ts.is_empty() {
            String::new()
        } else {
            format!(
                "[{}]",
                ts.iter()
                    .map(|t| t.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }

    fn type_decl(&mut self, t: &TypeDecl) -> Doc {
        let head = format!("type {}{} =", t.name.name, Self::tparams(&t.params));
        match &t.body {
            TypeBody::Alias(ty) => concat(vec![text(format!("{head} ")), self.type_doc(ty)]),
            TypeBody::Record(fields) => {
                let mut items = Vec::new();
                for f in fields {
                    let mut d = self.leading(f.name.span.start);
                    d.push(text(format!("{}: ", f.name.name)));
                    d.push(self.type_doc(&f.ty));
                    items.push(concat(d));
                }
                let sep = concat(vec![text(","), Doc::Line]);
                let trailing: Vec<Doc> = fields
                    .iter()
                    .map(|f| self.trailing(f.ty.span.end))
                    .collect();
                let items: Vec<Doc> = items
                    .into_iter()
                    .zip(trailing)
                    .map(|(i, t)| concat(vec![i, t]))
                    .collect();
                group(concat(vec![
                    text(format!("{head} {{")),
                    indent(concat(vec![Doc::Line, join(items, &sep)])),
                    Doc::Line,
                    text("}"),
                ]))
            }
            TypeBody::Adt(vs) => {
                let mut vdocs = Vec::new();
                let mut has_comment = false;
                for v in vs {
                    let lead = self.leading(v.span.start);
                    has_comment |= !lead.is_empty();
                    let mut s = v.name.name.clone();
                    let mut parts = Vec::new();
                    if !v.fields.is_empty() {
                        s.push('(');
                        parts.push(text(s.clone()));
                        let fs: Vec<Doc> = v.fields.iter().map(|f| self.type_doc(f)).collect();
                        parts = vec![text(v.name.name.clone()), text("(")];
                        parts.push(join(fs, &text(", ")));
                        parts.push(text(")"));
                    } else {
                        parts.push(text(s));
                    }
                    let tr = self.trailing(v.span.end);
                    has_comment |= !matches!(tr, Doc::Nil);
                    let mut all = lead;
                    all.extend(parts);
                    all.push(tr);
                    vdocs.push(concat(all));
                }
                let one_line = render(&join(vdocs.clone(), &text(" | ")), usize::MAX);
                if !has_comment && head.len() + 1 + one_line.chars().count() <= WIDTH {
                    return text(format!("{head} {one_line}"));
                }
                let mut parts = vec![text(head)];
                let mut inner = Vec::new();
                for d in vdocs {
                    inner.push(hard());
                    inner.push(text("| "));
                    inner.push(d);
                }
                parts.push(indent(concat(inner)));
                concat(parts)
            }
        }
    }

    fn fn_decl(&mut self, f: &FnDecl) -> Doc {
        let mut head = Vec::new();
        let mut kw = String::new();
        if f.is_extern {
            kw.push_str("extern ");
        }
        kw.push_str("fn ");
        if let Some(m) = &f.module {
            kw.push_str(&m.name);
            kw.push('.');
        }
        kw.push_str(&f.name.name);
        kw.push_str(&Self::tparams(&f.tparams));
        head.push(text(kw));
        let params: Vec<Doc> = f
            .params
            .iter()
            .map(|p| {
                concat(vec![
                    text(format!("{}: ", p.name.name)),
                    self.type_doc(&p.ty),
                ])
            })
            .collect();
        head.push(self.bracketed("(", params, ")"));
        head.push(text(" -> "));
        head.push(self.type_doc(&f.ret));

        let mut clauses: Vec<Doc> = Vec::new();
        for e in &f.requires {
            clauses.push(concat(vec![text("requires "), self.expr(e)]));
        }
        for e in &f.ensures {
            clauses.push(concat(vec![text("ensures "), self.expr(e)]));
        }
        if let Some(e) = &f.decreases {
            clauses.push(concat(vec![text("decreases "), self.expr(e)]));
        }
        let Some(body) = &f.body else {
            let mut parts = vec![concat(head)];
            for c in clauses {
                parts.push(indent(concat(vec![hard(), c])));
            }
            return concat(parts);
        };
        let body_doc = self.expr(body);
        if clauses.is_empty() {
            if block_like(body) {
                return concat(vec![concat(head), text(" = "), body_doc]);
            }
            return group(concat(vec![
                concat(head),
                text(" ="),
                indent(concat(vec![Doc::Line, body_doc])),
            ]));
        }
        let mut inner = Vec::new();
        for c in clauses {
            inner.push(hard());
            inner.push(indent_continuation(c));
        }
        inner.push(hard());
        inner.push(text("= "));
        inner.push(body_doc);
        concat(vec![concat(head), indent(concat(inner))])
    }

    fn const_decl(&mut self, c: &ConstDecl) -> Doc {
        let value = self.expr(&c.value);
        let prefix = concat(vec![
            text(format!("const {}: ", c.name.name)),
            self.type_doc(&c.ty),
            text(" ="),
        ]);
        if block_like(&c.value) {
            return concat(vec![prefix, text(" "), value]);
        }
        group(concat(vec![prefix, indent(concat(vec![Doc::Line, value]))]))
    }

    fn test_decl(&mut self, t: &TestDecl) -> Doc {
        let value = self.expr(&t.body);
        let prefix = text(format!("test {} =", t.name.name));
        if block_like(&t.body) {
            return concat(vec![prefix, text(" "), value]);
        }
        group(concat(vec![prefix, indent(concat(vec![Doc::Line, value]))]))
    }

    // ------------------------------------------------------------ types

    fn type_doc(&mut self, t: &TypeExpr) -> Doc {
        let base = match &t.kind {
            TypeKind::Unit => text("()"),
            TypeKind::Named { name, args } => {
                if args.is_empty() {
                    text(name.name.clone())
                } else {
                    let a: Vec<Doc> = args.iter().map(|x| self.type_doc(x)).collect();
                    concat(vec![
                        text(format!("{}[", name.name)),
                        join(a, &text(", ")),
                        text("]"),
                    ])
                }
            }
            TypeKind::Tuple(ts) => {
                let a: Vec<Doc> = ts.iter().map(|x| self.type_doc(x)).collect();
                concat(vec![text("("), join(a, &text(", ")), text(")")])
            }
            TypeKind::Fn(ps, r) => {
                let a: Vec<Doc> = ps.iter().map(|x| self.type_doc(x)).collect();
                concat(vec![
                    text("("),
                    join(a, &text(", ")),
                    text(") -> "),
                    self.type_doc(r),
                ])
            }
        };
        match &t.refine {
            Some(p) => concat(vec![base, text(" where "), self.expr(p)]),
            None => base,
        }
    }

    // ------------------------------------------------------------ expressions

    fn bracketed(&self, open: &str, items: Vec<Doc>, close: &str) -> Doc {
        if items.is_empty() {
            return text(format!("{open}{close}"));
        }
        let sep = concat(vec![text(","), Doc::Line]);
        group(concat(vec![
            text(open),
            indent(concat(vec![Doc::SoftLine, join(items, &sep)])),
            Doc::SoftLine,
            text(close),
        ]))
    }

    fn exprs(&mut self, es: &[Expr]) -> Vec<Doc> {
        es.iter().map(|e| self.expr(e)).collect()
    }

    fn fields(&mut self, fs: &[FieldInit]) -> Vec<Doc> {
        fs.iter()
            .map(|f| {
                concat(vec![
                    text(format!("{}: ", f.name.name)),
                    self.expr(&f.value),
                ])
            })
            .collect()
    }

    fn expr(&mut self, e: &Expr) -> Doc {
        match &e.kind {
            ExprKind::Int(s) | ExprKind::Float(s) => text(s.clone()),
            ExprKind::Str(s) => text(escape(s)),
            ExprKind::Bool(b) => text(b.to_string()),
            ExprKind::Unit => text("()"),
            ExprKind::Var(i) | ExprKind::Ctor(i) => text(i.name.clone()),
            ExprKind::Path { module, name } => text(format!("{}.{}", module.name, name.name)),
            ExprKind::Paren(x) => concat(vec![text("("), self.expr(x), text(")")]),
            ExprKind::List(xs) => {
                let items = self.exprs(xs);
                self.bracketed("[", items, "]")
            }
            ExprKind::Tuple(xs) => {
                let items = self.exprs(xs);
                self.bracketed("(", items, ")")
            }
            ExprKind::Record { ty, fields } => {
                let items = self.fields(fields);
                concat(vec![text(format!("{} ", ty.name)), self.braced(items)])
            }
            ExprKind::With { base, fields } => {
                let items = self.fields(fields);
                concat(vec![self.expr(base), text(" with "), self.braced(items)])
            }
            ExprKind::Unary(op, x) => {
                let o = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                };
                concat(vec![text(o), self.expr(x)])
            }
            ExprKind::Binary(..) => self.binary_chain(e),
            ExprKind::Call { callee, args } => {
                let a = self.exprs(args);
                concat(vec![self.expr(callee), self.bracketed("(", a, ")")])
            }
            ExprKind::MethodCall { .. } => self.method_chain(e),
            ExprKind::Field(b, n) => concat(vec![self.expr(b), text(format!(".{}", n.name))]),
            ExprKind::TupleField(b, i) => concat(vec![self.expr(b), text(format!(".{i}"))]),
            ExprKind::Index(b, i) => concat(vec![self.expr(b), text("["), self.expr(i), text("]")]),
            ExprKind::Lambda { params, body } => {
                let ps: Vec<Doc> = params
                    .iter()
                    .map(|p| match &p.ty {
                        Some(t) => {
                            concat(vec![text(format!("{}: ", p.name.name)), self.type_doc(t)])
                        }
                        None => text(p.name.name.clone()),
                    })
                    .collect();
                let head = if params.len() == 1 && params[0].ty.is_none() {
                    ps.into_iter().next().unwrap_or(Doc::Nil)
                } else {
                    concat(vec![text("("), join(ps, &text(", ")), text(")")])
                };
                concat(vec![head, text(" => "), self.expr(body)])
            }
            ExprKind::If { cond, then, els } => {
                let c = self.expr(cond);
                let t = self.block_branch(then);
                let mut parts = vec![text("if "), c, text(" "), t];
                if let Some(el) = els {
                    parts.push(text(" else "));
                    if matches!(el.kind, ExprKind::If { .. }) {
                        parts.push(self.expr(el));
                    } else {
                        parts.push(self.block_branch(el));
                    }
                }
                group(concat(parts))
            }
            ExprKind::Match { scrut, arms } => {
                let s = self.expr(scrut);
                let mut inner = Vec::new();
                for arm in arms {
                    inner.push(hard());
                    inner.push(self.arm(arm));
                }
                concat(vec![
                    text("match "),
                    s,
                    text(" {"),
                    indent(concat(inner)),
                    hard(),
                    text("}"),
                ])
            }
            ExprKind::Block { .. } => self.block_branch(e),
        }
    }

    fn braced(&self, items: Vec<Doc>) -> Doc {
        let sep = concat(vec![text(","), Doc::Line]);
        group(concat(vec![
            text("{"),
            indent(concat(vec![Doc::Line, join(items, &sep)])),
            Doc::Line,
            text("}"),
        ]))
    }

    fn arm(&mut self, a: &Arm) -> Doc {
        let mut parts = self.leading(a.span.start);
        parts.push(self.pat(&a.pat));
        if let Some(g) = &a.guard {
            parts.push(text(" if "));
            parts.push(self.expr(g));
        }
        parts.push(text(" => "));
        let body = self.expr(&a.body);
        if block_like(&a.body) {
            parts.push(body);
        } else {
            parts.push(group(indent(concat(vec![Doc::SoftLine, body]))));
        }
        parts.push(self.trailing(a.span.end));
        self.check_inside(a.span);
        concat(parts)
    }

    /// A `{ .. }` block (or the block of `if`/`else`).
    fn block_branch(&mut self, e: &Expr) -> Doc {
        match &e.kind {
            ExprKind::Block { stmts, tail } => {
                if stmts.is_empty() {
                    let lead_free = !self.comments.iter().enumerate().any(|(i, c)| {
                        !self.used[i] && c.span.start >= e.span.start && c.span.end <= e.span.end
                    });
                    if lead_free {
                        let t = self.expr(tail);
                        return group(concat(vec![
                            text("{"),
                            indent(concat(vec![Doc::Line, t])),
                            Doc::Line,
                            text("}"),
                        ]));
                    }
                }
                let mut inner = Vec::new();
                for s in stmts {
                    inner.push(hard());
                    let span = match s {
                        Stmt::Let { span, .. } | Stmt::Assert { span, .. } => *span,
                    };
                    inner.extend(self.leading(span.start));
                    inner.push(self.stmt(s));
                    inner.push(self.trailing(span.end));
                    self.check_inside(span);
                }
                inner.push(hard());
                inner.extend(self.leading(tail.span.start));
                inner.push(self.expr(tail));
                inner.push(self.trailing(tail.span.end));
                // comments before the closing brace
                let close = e.span.end;
                let rest = self.leading(close);
                if !rest.is_empty() {
                    let n = rest.len();
                    inner.push(hard());
                    inner.extend(rest.into_iter().take(n.saturating_sub(1)));
                }
                concat(vec![text("{"), indent(concat(inner)), hard(), text("}")])
            }
            _ => {
                let t = self.expr(e);
                group(concat(vec![
                    text("{"),
                    indent(concat(vec![Doc::Line, t])),
                    Doc::Line,
                    text("}"),
                ]))
            }
        }
    }

    fn stmt(&mut self, s: &Stmt) -> Doc {
        match s {
            Stmt::Let { pat, ty, init, .. } => {
                let mut parts = vec![text("let "), self.pat(pat)];
                if let Some(t) = ty {
                    parts.push(text(": "));
                    parts.push(self.type_doc(t));
                }
                parts.push(text(" = "));
                parts.push(self.expr(init));
                concat(parts)
            }
            Stmt::Assert { cond, .. } => concat(vec![text("assert "), self.expr(cond)]),
        }
    }

    fn binary_chain(&mut self, e: &Expr) -> Doc {
        let ExprKind::Binary(op, _, _) = &e.kind else {
            return Doc::Nil;
        };
        let class = op_class(*op);
        if class == 3 {
            // comparisons are non-associative: exactly two operands
            if let ExprKind::Binary(o, l, r) = &e.kind {
                let (a, b) = (self.expr(l), self.expr(r));
                return group(concat(vec![
                    a,
                    text(format!(" {}", o.as_str())),
                    indent(concat(vec![Doc::Line, b])),
                ]));
            }
        }
        // flatten left-assoc chains of the same precedence class
        let mut operands: Vec<&Expr> = Vec::new();
        let mut ops: Vec<BinOp> = Vec::new();
        let mut cur = e;
        loop {
            match &cur.kind {
                ExprKind::Binary(o, l, r) if op_class(*o) == class => {
                    operands.push(r);
                    ops.push(*o);
                    cur = l;
                }
                _ => {
                    operands.push(cur);
                    break;
                }
            }
        }
        operands.reverse();
        ops.reverse();
        let mut docs: Vec<Doc> = operands.iter().map(|x| self.expr(x)).collect();
        let first = docs.remove(0);
        let mut rest = Vec::new();
        for (o, d) in ops.iter().zip(docs) {
            if matches!(o, BinOp::And | BinOp::Or) {
                rest.push(concat(vec![Doc::Line, text(format!("{} ", o.as_str())), d]));
            } else {
                rest.push(concat(vec![text(format!(" {}", o.as_str())), Doc::Line, d]));
            }
        }
        group(concat(vec![first, indent(concat(rest))]))
    }

    fn method_chain(&mut self, e: &Expr) -> Doc {
        // collect `base.m1(..).m2(..)`
        let mut calls: Vec<(&tez_syntax::ast::Ident, &Vec<Expr>)> = Vec::new();
        let mut cur = e;
        while let ExprKind::MethodCall { recv, name, args } = &cur.kind {
            calls.push((name, args));
            cur = recv;
        }
        calls.reverse();
        let base = self.expr(cur);
        let mut links = Vec::new();
        for (name, args) in &calls {
            let a = self.exprs(args);
            links.push(concat(vec![
                text(format!(".{}", name.name)),
                self.bracketed("(", a, ")"),
            ]));
        }
        if calls.len() < 2 {
            return concat(vec![base, concat(links)]);
        }
        let broken: Vec<Doc> = links
            .into_iter()
            .map(|l| concat(vec![Doc::SoftLine, l]))
            .collect();
        concat(vec![base, group(indent(concat(broken)))])
    }

    // ------------------------------------------------------------ patterns

    fn pat(&self, p: &Pat) -> Doc {
        match &p.kind {
            PatKind::Wild => text("_"),
            PatKind::Unit => text("()"),
            PatKind::Bind(i) => text(i.name.clone()),
            PatKind::Lit(l) => text(match l {
                Lit::Int(s) | Lit::Float(s) => s.clone(),
                Lit::Str(s) => escape(s),
                Lit::Bool(b) => b.to_string(),
            }),
            PatKind::Ctor { name, args } => {
                if args.is_empty() {
                    text(name.name.clone())
                } else {
                    let a: Vec<Doc> = args.iter().map(|x| self.pat(x)).collect();
                    concat(vec![
                        text(format!("{}(", name.name)),
                        join(a, &text(", ")),
                        text(")"),
                    ])
                }
            }
            PatKind::Tuple(ps) => {
                let a: Vec<Doc> = ps.iter().map(|x| self.pat(x)).collect();
                concat(vec![text("("), join(a, &text(", ")), text(")")])
            }
            PatKind::List { items, rest } => {
                let mut a: Vec<Doc> = items.iter().map(|x| self.pat(x)).collect();
                match rest {
                    Some(Some(id)) => a.push(text(format!("..{}", id.name))),
                    Some(None) => a.push(text("..")),
                    None => {}
                }
                concat(vec![text("["), join(a, &text(", ")), text("]")])
            }
        }
    }
}

/// Bodies printed on the same line as `=`: their own layout starts with `{`/`match`/`if`.
fn block_like(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Match { .. } => true,
        ExprKind::Block { stmts, .. } => !stmts.is_empty(),
        ExprKind::If { .. } => true,
        _ => false,
    }
}

fn indent_continuation(d: Doc) -> Doc {
    d
}

fn op_class(op: BinOp) -> u8 {
    match op {
        BinOp::Or => 0,
        BinOp::And => 1,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
        BinOp::Concat => 4,
        BinOp::Add | BinOp::Sub => 5,
        BinOp::Mul | BinOp::Div | BinOp::Rem => 6,
    }
}
