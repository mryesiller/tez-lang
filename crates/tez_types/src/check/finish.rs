//! End-of-frame work: deferred constraints, type finalization, termination, patterns.

use tez_diag::Code;
use tez_syntax::{ByteSpan, Problem};

use super::{Checker, Pending};
use crate::exhaust::{self, ArmInfo, Issue};
use crate::hir::{Expr, ExprKind, Stmt};
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
