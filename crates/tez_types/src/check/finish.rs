//! End-of-frame work: deferred constraints, type finalization, termination, patterns.

use tez_diag::Code;
use tez_syntax::{ByteSpan, Problem};

use super::{Checker, Pending};
use crate::exhaust::{self, ArmInfo, Issue};
use crate::hir::{Expr, ExprKind, Pat, PatKind, Stmt, TypeKind};
use crate::ty::Ty;

impl Checker<'_> {
    /// Resolves deferred operator constraints, then replaces inference variables in `exprs`.
    pub fn finish_frame_exprs<'e>(&mut self, exprs: impl Iterator<Item = &'e mut Expr>) {
        self.process_pending();
        for e in exprs {
            e.visit_tys_mut(&mut |t, span| self.zonk(t, span));
        }
    }

    fn ambiguous(&mut self, span: ByteSpan) {
        if !self.ambiguous_reported {
            self.ambiguous_reported = true;
            self.err(
                Problem::new(Code::TypeAmbiguous, "cannot infer this type", span)
                    .expected("a type annotation, e.g. `let xs: List[Int] = []`"),
            );
        }
    }

    fn zonk(&mut self, ty: &mut Ty, span: ByteSpan) {
        let r = self.resolve(ty);
        if r.has_var() {
            self.ambiguous(span);
            *ty = r.map(&mut |t| matches!(t, Ty::Var(_)).then_some(Ty::Error));
        } else {
            *ty = r;
        }
    }

    fn process_pending(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for p in pending {
            match p {
                Pending::Numeric {
                    ty,
                    span,
                    op,
                    int_only,
                } => match self.resolve(&ty) {
                    Ty::Int | Ty::Error => {}
                    Ty::Float if !int_only => {}
                    Ty::Var(_) => self.ambiguous(span),
                    other => {
                        let want = if int_only { "Int" } else { "Int or Float" };
                        let found = self.show(&other);
                        self.err(
                            Problem::new(
                                Code::TypeBadOperand,
                                format!("`{op}` needs {want}, found `{found}`"),
                                span,
                            )
                            .expected(want),
                        );
                    }
                },
                Pending::Ordered { ty, span } => match self.resolve(&ty) {
                    Ty::Int | Ty::Float | Ty::Str | Ty::Error => {}
                    Ty::Var(_) => self.ambiguous(span),
                    other => {
                        let found = self.show(&other);
                        self.err(
                            Problem::new(
                                Code::TypeNotOrdered,
                                format!("`{found}` cannot be ordered with `<`"),
                                span,
                            )
                            .expected("Int, Float or Str"),
                        );
                    }
                },
                Pending::Equatable { ty, span } => {
                    let t = self.resolve(&ty);
                    if t.has_var() {
                        self.ambiguous(span);
                    } else {
                        self.check_equatable(&t, span);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ termination

    pub fn check_termination(&mut self) {
        let n = self.program.fns.len();
        let edges = self.call_edges.clone();
        let mut index = vec![usize::MAX; n];
        let mut low = vec![0usize; n];
        let mut on_stack = vec![false; n];
        let mut stack: Vec<usize> = Vec::new();
        let mut counter = 0usize;
        let mut sccs: Vec<Vec<usize>> = Vec::new();

        #[allow(clippy::too_many_arguments)]
        fn strong(
            v: usize,
            edges: &[Vec<usize>],
            index: &mut [usize],
            low: &mut [usize],
            on_stack: &mut [bool],
            stack: &mut Vec<usize>,
            counter: &mut usize,
            sccs: &mut Vec<Vec<usize>>,
        ) {
            index[v] = *counter;
            low[v] = *counter;
            *counter += 1;
            stack.push(v);
            on_stack[v] = true;
            for &w in &edges[v] {
                if index[w] == usize::MAX {
                    strong(w, edges, index, low, on_stack, stack, counter, sccs);
                    low[v] = low[v].min(low[w]);
                } else if on_stack[w] {
                    low[v] = low[v].min(index[w]);
                }
            }
            if low[v] == index[v] {
                let mut comp = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack[w] = false;
                    comp.push(w);
                    if w == v {
                        break;
                    }
                }
                sccs.push(comp);
            }
        }

        for v in 0..n {
            if index[v] == usize::MAX {
                strong(
                    v,
                    &edges,
                    &mut index,
                    &mut low,
                    &mut on_stack,
                    &mut stack,
                    &mut counter,
                    &mut sccs,
                );
            }
        }
        for comp in sccs {
            let cyclic = comp.len() > 1 || edges[comp[0]].contains(&comp[0]);
            if !cyclic {
                continue;
            }
            for &f in &comp {
                self.program.fns[f].is_recursive = true;
                // structural recursion needs no clause: the measure is inferred (and still proved by the verifier)
                if comp.len() == 1
                    && self.program.fns[f].decreases.is_none()
                    && !self.program.fns[f].is_prelude
                    && let Some(m) = self.infer_measure(f)
                {
                    self.program.fns[f].decreases = Some(m);
                }
                if self.program.fns[f].decreases.is_none() && !self.program.fns[f].is_prelude {
                    let span = self.fn_srcs[f]
                        .as_ref()
                        .map_or(self.program.fns[f].span, |s| s.decl.name.span);
                    let name = self.program.fns[f].name.clone();
                    self.problems.push(
                        Problem::new(
                            Code::TerminationMissing,
                            format!("`{name}` is recursive and needs a `decreases` clause"),
                            span,
                        )
                        .expected("`decreases <Int expression that shrinks>`"),
                    );
                }
            }
        }
    }

    /// A `decreases` measure for a self-recursive function whose every recursive call passes, for one parameter, a
    /// value taken apart from that parameter by a pattern: the tail of `[h, ..t]` (measure `p.len()`) or a field of a
    /// constructor of a sum type (measure `p`, structural size). `None` when no parameter qualifies.
    fn infer_measure(&self, f: usize) -> Option<Expr> {
        let def = &self.program.fns[f];
        let body = def.body.as_ref()?;
        for p in &def.params {
            let is_list = matches!(p.ty, Ty::List(_));
            let is_sum = matches!(&p.ty, Ty::Adt(id, _) if matches!(self.program.types[*id].kind, TypeKind::Adt(_)));
            if !is_list && !is_sum {
                continue;
            }
            // locals that are proper parts of the parameter (transitively)
            let mut smaller: Vec<u32> = Vec::new();
            let mut origin: Vec<u32> = vec![p.local];
            loop {
                let before = smaller.len();
                body.walk(&mut |e| {
                    if let ExprKind::Match(scrut, arms) = &e.kind
                        && let ExprKind::Local(s) = scrut.kind
                        && (origin.contains(&s))
                    {
                        for arm in arms {
                            collect_parts(&arm.pat, is_list, &p.ty, &mut smaller);
                        }
                    }
                });
                for s in &smaller {
                    if !origin.contains(s) {
                        origin.push(*s);
                    }
                }
                if smaller.len() == before {
                    break;
                }
                smaller.sort_unstable();
                smaller.dedup();
                if smaller.len() == before {
                    break;
                }
            }
            let at = def.params.iter().position(|q| q.local == p.local)?;
            let mut calls = 0usize;
            let mut all_smaller = true;
            body.walk(&mut |e| {
                if let ExprKind::Call { f: g, args, .. } = &e.kind
                    && *g == f
                {
                    calls += 1;
                    let ok = matches!(args.get(at).map(|a| &a.kind), Some(ExprKind::Local(l)) if smaller.contains(l));
                    all_smaller &= ok;
                }
            });
            if calls == 0 || !all_smaller {
                continue;
            }
            let span = def.span;
            let local = Expr {
                kind: ExprKind::Local(p.local),
                ty: p.ty.clone(),
                span,
            };
            if is_sum {
                return Some(local);
            }
            let len = self
                .program
                .fns
                .iter()
                .position(|d| d.is_prelude && d.full_name() == "List.len")?;
            let Ty::List(elem) = &p.ty else { return None };
            return Some(Expr {
                kind: ExprKind::Call {
                    f: len,
                    targs: vec![(**elem).clone()],
                    args: vec![local],
                },
                ty: Ty::Int,
                span,
            });
        }
        None
    }

    // ------------------------------------------------------------ patterns

    pub fn check_patterns(&mut self) {
        let mut issues: Vec<Problem> = Vec::new();
        {
            let prog = &self.program;
            let mut visit = |e: &Expr| match &e.kind {
                ExprKind::Match(scrut, arms) => {
                    let infos: Vec<ArmInfo<'_>> = arms
                        .iter()
                        .map(|a| ArmInfo {
                            pat: &a.pat,
                            guarded: a.guard.is_some(),
                        })
                        .collect();
                    for (i, issue) in exhaust::check_arms(prog, &scrut.ty, &infos) {
                        match issue {
                            Issue::NonExhaustive(w) => issues.push(
                                Problem::new(
                                    Code::MatchNonExhaustive,
                                    format!("this `match` does not cover `{w}`"),
                                    e.span,
                                )
                                .expected(format!("an arm for `{w}`")),
                            ),
                            Issue::Unreachable => {
                                let sp = i.map_or(e.span, |i| arms[i].pat.span);
                                issues.push(
                                    Problem::new(
                                        Code::MatchUnreachable,
                                        "this arm can never match",
                                        sp,
                                    )
                                    .expected("delete the arm"),
                                );
                            }
                        }
                    }
                }
                ExprKind::Block(stmts, _) => {
                    for s in stmts {
                        if let Stmt::Let { pat, .. } = s
                            && let Some(w) = exhaust::check_irrefutable(prog, pat)
                        {
                            issues.push(
                                Problem::new(
                                    Code::MatchNonExhaustive,
                                    format!("this `let` pattern does not cover `{w}`"),
                                    pat.span,
                                )
                                .expected("an irrefutable pattern, or use `match`"),
                            );
                        }
                    }
                }
                _ => {}
            };
            for f in &prog.fns {
                if f.is_prelude && false {
                    continue;
                }
                for e in f
                    .requires
                    .iter()
                    .chain(&f.ensures)
                    .chain(&f.decreases)
                    .chain(&f.body)
                {
                    e.walk(&mut visit);
                }
            }
            for c in &prog.consts {
                c.value.walk(&mut visit);
            }
            for t in &prog.tests {
                t.body.walk(&mut visit);
            }
        }
        for p in issues {
            self.err(p);
        }
    }
}

/// Locals bound in `pat` to a proper part of the matched value: the rest of a list pattern with at least one leading
/// item, or (for sum types) any binding inside a constructor's arguments that has the same type as the whole.
fn collect_parts(pat: &Pat, is_list: bool, whole: &Ty, out: &mut Vec<u32>) {
    match &pat.kind {
        PatKind::List { items, rest } if is_list => {
            if !items.is_empty()
                && let Some(Some(t)) = rest
            {
                out.push(*t);
            }
        }
        PatKind::Ctor { args, .. } if !is_list => {
            for a in args {
                bindings_of_type(a, whole, out);
            }
        }
        _ => {}
    }
}

fn bindings_of_type(pat: &Pat, whole: &Ty, out: &mut Vec<u32>) {
    match &pat.kind {
        PatKind::Bind(l) if &pat.ty == whole => out.push(*l),
        PatKind::Ctor { args, .. } => args.iter().for_each(|a| bindings_of_type(a, whole, out)),
        PatKind::Tuple(ps) => ps.iter().for_each(|a| bindings_of_type(a, whole, out)),
        _ => {}
    }
}
