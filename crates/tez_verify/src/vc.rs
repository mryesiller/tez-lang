//! Verification-condition generation by symbolic evaluation of the typed HIR.
//!
//! Evaluating an expression yields an SMT term for its value. Along the way the evaluator
//! records *obligations* (a goal plus the path conditions under which it must hold) and
//! *facts* (assumptions that are true of every execution). Soundness rules:
//!
//! * facts are either definitional (`v = term` for a fresh `v`), true of the built-in types'
//!   run-time semantics, or guarded by the path condition under which they were established;
//! * anything the solver cannot prove is reported, never assumed;
//! * calls inline non-recursive callees (exact semantics) or fall back to the callee's
//!   contract after proving its preconditions.

use std::collections::HashMap;

use num_bigint::BigInt;
use num_traits::Signed;
use tez_syntax::ByteSpan;
use tez_types::Ty;
use tez_types::hir::{
    BinOp, Expr, ExprKind, FnId, LitVal, LocalInfo, Pat, PatKind, Program, Refinement, Stmt, UnOp,
};

use crate::enc::Enc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    DivZero,
    FloatDomain,
    IndexBounds,
    Precondition,
    Postcondition,
    Refinement,
    Assert,
    Termination,
}

pub(crate) struct Obligation {
    pub kind: Kind,
    pub span: ByteSpan,
    pub msg: String,
    pub expected: Option<String>,
    pub path: Vec<String>,
    pub goal: String,
    pub facts_len: usize,
    pub watch: Vec<(String, String)>,
}

type Env = Vec<String>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LamState {
    /// Created for an inlined callee; its body is checked where it is applied.
    Deferred,
    /// Its body has been checked for all arguments.
    Verified,
}

/// A lambda the evaluator knows about, keyed by the constant standing for it.
struct LamInfo<'p> {
    expr: &'p Expr,
    env: Env,
    tmap: Vec<(String, Ty)>,
    path: Vec<String>,
    state: LamState,
    applied: bool,
    emit: bool,
}

/// Built-in higher-order functions: `(lambda argument, element parameter)`.
fn hof_info(name: &str) -> Option<(usize, usize)> {
    match name {
        "List.map" | "List.filter" | "List.any" | "List.all" | "List.count" | "List.flatMap"
        | "List.sortBy" => Some((1, 0)),
        "List.fold" => Some((2, 1)),
        _ => None,
    }
}

pub(crate) struct Vc<'p> {
    prog: &'p Program,
    src: &'p str,
    pub enc: Enc<'p>,
    pub facts: Vec<String>,
    pub obligs: Vec<Obligation>,
    emit: bool,
    tmap: Vec<(String, Ty)>,
    reach: Vec<Vec<bool>>,
    cur_fn: Option<FnId>,
    cur_measure: Option<String>,
    inline_depth: usize,
    inline_budget: i64,
    watch: Vec<(String, String)>,
    watch_on: bool,
    const_terms: Vec<Option<String>>,
    path: Vec<String>,
    div_ready: bool,
    closed: bool,
    closed_fail: bool,
    lams: HashMap<String, LamInfo<'p>>,
    pub term_flags: HashMap<FnId, String>,
}

fn int_lit(n: &BigInt) -> String {
    if n.is_negative() {
        format!("(- {})", n.abs())
    } else {
        n.to_string()
    }
}

fn and_all(ts: &[String]) -> String {
    match ts.len() {
        0 => "true".into(),
        1 => ts[0].clone(),
        _ => format!("(and {})", ts.join(" ")),
    }
}

fn atomic(t: &str) -> bool {
    !t.starts_with('(') || (t.starts_with("(- ") && t.matches('(').count() == 1)
}

impl<'p> Vc<'p> {
    #[allow(clippy::needless_range_loop)]
    pub fn new(prog: &'p Program, src: &'p str) -> Self {
        let n = prog.fns.len();
        let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, f) in prog.fns.iter().enumerate() {
            for e in f
                .requires
                .iter()
                .chain(&f.ensures)
                .chain(&f.decreases)
                .chain(&f.body)
            {
                e.walk(&mut |x| match &x.kind {
                    ExprKind::Call { f: g, .. } | ExprKind::FnRef(g, _) => edges[i].push(*g),
                    _ => {}
                });
            }
        }
        let mut reach = vec![vec![false; n]; n];
        for s in 0..n {
            let mut stack = vec![s];
            while let Some(v) = stack.pop() {
                for &w in &edges[v] {
                    if !reach[s][w] {
                        reach[s][w] = true;
                        stack.push(w);
                    }
                }
            }
        }
        Self {
            prog,
            src,
            enc: Enc::new(prog),
            facts: Vec::new(),
            obligs: Vec::new(),
            emit: true,
            tmap: Vec::new(),
            reach,
            cur_fn: None,
            cur_measure: None,
            inline_depth: 0,
            inline_budget: 0,
            watch: Vec::new(),
            watch_on: true,
            const_terms: vec![None; prog.consts.len()],
            path: Vec::new(),
            div_ready: false,
            closed: false,
            closed_fail: false,
            lams: HashMap::new(),
            term_flags: HashMap::new(),
        }
    }

    // ------------------------------------------------------------ helpers

    fn ty(&self, t: &Ty) -> Ty {
        if self.tmap.is_empty() {
            t.clone()
        } else {
            t.subst_params(&self.tmap)
        }
    }

    fn sort(&mut self, t: &Ty) -> String {
        let t = self.ty(t);
        self.enc.sort(&t)
    }

    fn fact(&mut self, t: String) {
        if !self.closed {
            self.facts.push(t);
        }
    }

    fn guarded_fact(&mut self, t: String) {
        if self.closed {
            return;
        }
        if self.path.is_empty() {
            self.facts.push(t);
        } else {
            self.facts.push(format!("(=> {} {t})", and_all(&self.path)));
        }
    }

    /// Names a compound term with a fresh constant (keeps terms small).
    fn def(&mut self, ty: &Ty, term: String) -> String {
        if atomic(&term) || self.closed {
            return term;
        }
        let s = self.sort(ty);
        let v = self.enc.declare_const("v", &s);
        self.facts.push(format!("(= {v} {term})"));
        v
    }

    /// Like [`Vc::def`] for a type that is already concrete (no type-parameter substitution).
    fn def_concrete(&mut self, ty: &Ty, term: String) -> String {
        if atomic(&term) || self.closed {
            return term;
        }
        let s = self.enc.sort(ty);
        let v = self.enc.declare_const("v", &s);
        self.facts.push(format!("(= {v} {term})"));
        v
    }

    fn obligation(
        &mut self,
        kind: Kind,
        span: ByteSpan,
        msg: String,
        expected: Option<String>,
        goal: String,
    ) {
        if !self.emit || self.closed {
            return;
        }
        self.obligs.push(Obligation {
            kind,
            span,
            msg,
            expected,
            path: self.path.clone(),
            goal,
            facts_len: self.facts.len(),
            watch: self.watch.clone(),
        });
    }

    fn src_text(&self, span: ByteSpan, prelude: bool) -> String {
        if prelude {
            String::new()
        } else {
            span.text(self.src).trim().to_string()
        }
    }

    fn ensure_div(&mut self) {
        if self.div_ready {
            return;
        }
        self.div_ready = true;
        self.enc.push_decl(
            "(define-fun tdiv ((a Int) (b Int)) Int (ite (>= a 0) (ite (> b 0) (div a b) (- (div a (- b)))) (ite (> b 0) (- (div (- a) b)) (div (- a) (- b)))))"
                .into(),
        );
        self.enc
            .push_decl("(define-fun trem ((a Int) (b Int)) Int (- a (* b (tdiv a b))))".into());
    }

    fn bind_watch(&mut self, name: &str, ty: &Ty, term: &str) {
        if !self.watch_on || name.is_empty() || self.watch.len() >= 14 {
            return;
        }
        let ty = self.ty(ty);
        match ty {
            Ty::Int | Ty::Bool => self.watch.push((name.to_string(), term.to_string())),
            Ty::List(_) => {
                let ops = self.enc.list_ops(&ty);
                self.watch
                    .push((format!("{name}.len()"), format!("({} {term})", ops.len)));
            }
            Ty::Str => self
                .watch
                .push((format!("{name}.len()"), format!("(slen {term})"))),
            _ => {}
        }
    }

    /// Value of a `decreases` expression: an `Int`, or the structural rank of a data value.
    fn measure(&mut self, e: &'p Expr, env: &mut Env) -> String {
        let t = self.eval(e, env);
        let ty = self.ty(&e.ty);
        if matches!(ty, Ty::Adt(..)) {
            let r = self.enc.rank_fn(&ty);
            return format!("({r} {t})");
        }
        t
    }

    /// The Boolean that says "`f` terminates" (asserted once its measure is proved).
    fn term_flag(&mut self, f: FnId) -> String {
        if let Some(n) = self.term_flags.get(&f) {
            return n.clone();
        }
        let n = self.enc.declare_const("T", "Bool");
        self.term_flags.insert(f, n.clone());
        n
    }

    /// Strongly connected components of the call graph, callees before callers.
    pub fn scc_order(&self) -> Vec<Vec<FnId>> {
        let n = self.prog.fns.len();
        let mut done = vec![false; n];
        let mut out: Vec<Vec<FnId>> = Vec::new();
        // f is "below" g when g reaches f; emit components smallest-reach first
        let mut order: Vec<FnId> = (0..n).collect();
        order.sort_by_key(|&f| self.reach[f].iter().filter(|b| **b).count());
        for &f in &order {
            if done[f] {
                continue;
            }
            let comp: Vec<FnId> = (0..n)
                .filter(|&g| g == f || (self.reach[f][g] && self.reach[g][f]))
                .collect();
            for &g in &comp {
                done[g] = true;
            }
            out.push(comp);
        }
        out
    }

    fn same_scc(&self, a: FnId, b: FnId) -> bool {
        a == b && self.reach[a][a] || (self.reach[a][b] && self.reach[b][a])
    }

    // ------------------------------------------------------------ driver entry points

    /// Verifies constant initialisers (once, before functions that use them).
    pub fn verify_consts(&mut self) {
        for c in 0..self.prog.consts.len() {
            let def = &self.prog.consts[c];
            self.emit = true;
            self.cur_fn = None;
            self.path.clear();
            self.watch.clear();
            self.watch_on = false;
            let mut env: Env = vec![String::new(); def.nlocals as usize];
            let v = self.eval(&def.value, &mut env);
            let tp = def.ty.clone();
            for r in &def.refine {
                env[r.it as usize].clone_from(&v);
                let goal = self.eval(&r.pred, &mut env);
                self.obligation(
                    Kind::Refinement,
                    r.pred.span,
                    "the constant may not satisfy its refinement".into(),
                    None,
                    goal.clone(),
                );
                self.fact(goal);
            }
            let _ = tp;
            self.const_terms[c] = Some(v);
        }
    }

    /// Checks that type-level refinement predicates are well-defined (e.g. no division by 0).
    pub fn verify_type_preds(&mut self) {
        for tid in 0..self.prog.types.len() {
            let def = &self.prog.types[tid];
            if def.is_prelude {
                continue;
            }
            let map: Vec<(String, Ty)> = def
                .params
                .iter()
                .map(|p| (p.clone(), Ty::Param(p.clone())))
                .collect();
            self.tmap = map;
            let variants: Vec<&[tez_types::hir::FieldDef]> = (0..def.variant_count())
                .filter_map(|v| def.variant_fields(v))
                .collect();
            for fields in variants {
                for f in fields {
                    for r in &f.refine {
                        self.emit = true;
                        self.cur_fn = None;
                        self.path.clear();
                        self.watch.clear();
                        self.watch_on = false;
                        let s = self.sort(&f.ty);
                        let subject = self.enc.declare_const("it", &s);
                        let mut env: Env = vec![String::new(); r.nlocals.max(r.it + 1) as usize];
                        env[r.it as usize] = subject;
                        let _ = self.eval(&r.pred, &mut env);
                    }
                }
            }
            self.tmap.clear();
        }
    }

    pub fn verify_fn(&mut self, id: FnId) {
        let prog = self.prog;
        let def = &prog.fns[id];
        let Some(body) = &def.body else { return };
        self.cur_fn = Some(id);
        self.cur_measure = None;
        self.tmap = def
            .tparams
            .iter()
            .map(|p| (p.clone(), Ty::Param(p.clone())))
            .collect();
        self.emit = true;
        self.path.clear();
        self.watch.clear();
        self.watch_on = true;
        self.inline_depth = 0;
        self.inline_budget = 400;
        self.lams.clear();
        let mut env: Env = vec![String::new(); def.nlocals as usize];
        for p in &def.params {
            let s = self.sort(&p.ty);
            let c = self.enc.declare_const("p", &s);
            env[p.local as usize].clone_from(&c);
            self.bind_watch(&p.name, &p.ty, &c);
        }
        // Entry assumptions live in the path (not in the global facts): every obligation of
        // the function then sees them, and slicing can never drop them.
        for p in &def.params {
            for r in &p.refine {
                let subject = env[p.local as usize].clone();
                env[r.it as usize] = subject;
                let t = self.eval(&r.pred, &mut env);
                self.path.push(t);
            }
        }
        for r in &def.requires {
            let t = self.eval(r, &mut env);
            self.path.push(t);
        }
        if let Some(d) = &def.decreases {
            let m = self.measure(d, &mut env);
            self.cur_measure = Some(m);
        }
        let result = self.eval(body, &mut env);
        self.escape(&result);
        env[def.result_local as usize].clone_from(&result);
        for r in &def.ret_refine {
            env[r.it as usize].clone_from(&result);
            let goal = self.eval(&r.pred, &mut env);
            let text = self.src_text(r.pred.span, def.is_prelude);
            self.obligation(
                Kind::Refinement,
                r.pred.span,
                format!(
                    "the result of `{}` may not satisfy its refined return type",
                    def.name
                ),
                (!text.is_empty()).then(|| format!("`where {text}`")),
                goal,
            );
        }
        for e in &def.ensures {
            let goal = self.eval(e, &mut env);
            let text = self.src_text(e.span, def.is_prelude);
            self.obligation(
                Kind::Postcondition,
                e.span,
                format!("the result of `{}` may violate its `ensures`", def.name),
                (!text.is_empty()).then(|| format!("`ensures {text}`")),
                goal,
            );
        }
        // lambdas that were never applied nor passed on must still be sound for all arguments
        let dead: Vec<String> = self
            .lams
            .iter()
            .filter(|(_, i)| i.state == LamState::Deferred && !i.applied)
            .map(|(k, _)| k.clone())
            .collect();
        for k in dead {
            self.escape(&k);
        }
        self.cur_fn = None;
        self.tmap.clear();
    }

    // ------------------------------------------------------------ expressions

    pub fn eval(&mut self, e: &'p Expr, env: &mut Env) -> String {
        let ty = e.ty.clone();
        match &e.kind {
            ExprKind::Int(n) => int_lit(n),
            ExprKind::Float(f) => self.enc.float_lit(*f),
            ExprKind::Str(s) => self.enc.str_lit(s),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Unit => self.enc.unit_val(),
            ExprKind::Local(i) => env[*i as usize].clone(),
            ExprKind::Const(c) => self.const_terms[*c].clone().unwrap_or_else(|| "0".into()),
            ExprKind::FnRef(f, targs) => self.fn_value(*f, targs, &ty, e.span),
            ExprKind::Call { f, targs, args } => self.call(*f, targs, args, &ty, e.span, env),
            ExprKind::CallValue { callee, args } => self.call_value(callee, args, &ty, env),
            ExprKind::Ctor {
                ty: tid,
                variant,
                targs,
                args,
            } => {
                let terms: Vec<String> = args.iter().map(|a| self.eval(a, env)).collect();
                for t in &terms {
                    self.escape(t);
                }
                let adt = Ty::Adt(*tid, targs.clone());
                self.ctor_term(&adt, *variant, &terms, args)
            }
            ExprKind::Field(b, i) => {
                let t = self.eval(b, env);
                let bty = self.ty(&b.ty);
                let sel = self.enc.selector(&bty, 0, *i);
                let v = self.def(&ty, format!("({sel} {t})"));
                self.assume_field_refinements(&bty, 0, *i, &v, None);
                v
            }
            ExprKind::TupleField(b, i) => {
                let t = self.eval(b, env);
                let bty = self.ty(&b.ty);
                let sel = self.enc.selector(&bty, 0, *i);
                self.def(&ty, format!("({sel} {t})"))
            }
            ExprKind::With(b, ups) => {
                let base = self.eval(b, env);
                let bty = self.ty(&b.ty);
                let Ty::Adt(tid, _) = &bty else { return base };
                let nfields = self.prog.types[*tid]
                    .variant_fields(0)
                    .map_or(0, <[_]>::len);
                let mut terms = Vec::new();
                let mut exprs: Vec<Option<&Expr>> = vec![None; nfields];
                #[allow(clippy::needless_range_loop)]
                for j in 0..nfields {
                    if let Some((_, ue)) = ups.iter().find(|(k, _)| *k == j) {
                        terms.push(self.eval(ue, env));
                        exprs[j] = Some(ue);
                    } else {
                        let sel = self.enc.selector(&bty, 0, j);
                        terms.push(format!("({sel} {base})"));
                    }
                }
                self.ctor_term_with(&bty, 0, &terms, &exprs)
            }
            ExprKind::Tuple(es) => {
                let terms: Vec<String> = es.iter().map(|x| self.eval(x, env)).collect();
                for t in &terms {
                    self.escape(t);
                }
                let c = self.enc.ctor(&self.ty(&ty), 0);
                self.def(&ty, format!("({c} {})", terms.join(" ")))
            }
            ExprKind::List(es) => {
                let terms: Vec<String> = es.iter().map(|x| self.eval(x, env)).collect();
                for t in &terms {
                    self.escape(t);
                }
                let lt = self.ty(&ty);
                let ops = self.enc.list_ops(&lt);
                let mut acc = ops.nil.clone();
                for t in terms {
                    acc = self.def(&ty, format!("({} {acc} {t})", ops.snoc));
                }
                acc
            }
            ExprKind::Index(b, i) => {
                let l = self.eval(b, env);
                let ix = self.eval(i, env);
                let lt = self.ty(&b.ty);
                let ops = self.enc.list_ops(&lt);
                let goal = format!("(and (<= 0 {ix}) (< {ix} ({} {l})))", ops.len);
                let text = self.src_text(i.span, false);
                self.obligation(
                    Kind::IndexBounds,
                    i.span,
                    "the index may be out of range".into(),
                    Some(format!(
                        "`0 <= {text} && {text} < {}.len()`",
                        self.src_text(b.span, false)
                    )),
                    goal,
                );
                self.def(&ty, format!("({} {l} {ix})", ops.get))
            }
            ExprKind::Unary(op, x) => {
                let t = self.eval(x, env);
                let r = match (op, self.ty(&x.ty)) {
                    (UnOp::Not, _) => format!("(not {t})"),
                    (UnOp::Neg, Ty::Float) => format!("(fneg {t})"),
                    (UnOp::Neg, _) => format!("(- {t})"),
                };
                self.def(&ty, r)
            }
            ExprKind::Binary(op, l, r) => self.binary(*op, l, r, &ty, env),
            ExprKind::If(c, t, f) => {
                let cond = self.eval(c, env);
                let mark = self.watch.len();
                self.path.push(cond.clone());
                let tv = self.eval(t, env);
                self.path.pop();
                self.path.push(format!("(not {cond})"));
                let fv = self.eval(f, env);
                self.path.pop();
                self.watch.truncate(mark);
                self.def(&ty, format!("(ite {cond} {tv} {fv})"))
            }
            ExprKind::Match(s, arms) => self.match_expr(s, arms, &ty, env),
            ExprKind::Block(stmts, tail) => {
                let mark = self.watch.len();
                let path_len = self.path.len();
                for st in stmts {
                    match st {
                        Stmt::Let {
                            pat, init, refine, ..
                        } => {
                            let v = self.eval(init, env);
                            for r in refine {
                                env[r.it as usize].clone_from(&v);
                                let goal = self.eval(&r.pred, env);
                                let text = self.src_text(r.pred.span, false);
                                self.obligation(
                                    Kind::Refinement,
                                    init.span,
                                    "the value may not satisfy its refined type".into(),
                                    Some(format!("`where {text}`")),
                                    goal.clone(),
                                );
                                self.guarded_fact(goal);
                            }
                            let _ = self.pat_cond(pat, &v, env);
                        }
                        Stmt::Assert(c) => {
                            let g = self.eval(c, env);
                            let text = self.src_text(c.span, false);
                            self.obligation(
                                Kind::Assert,
                                c.span,
                                "the assertion may fail".into(),
                                Some(format!("`{text}` must hold here")),
                                g.clone(),
                            );
                            self.path.push(g);
                        }
                    }
                }
                let v = self.eval(tail, env);
                self.path.truncate(path_len);
                self.watch.truncate(mark);
                v
            }
            ExprKind::Lambda { .. } => {
                if self.closed {
                    self.closed_fail = true;
                    return "0".into();
                }
                self.lambda_generic(e, env)
            }
        }
    }

    fn binary(&mut self, op: BinOp, l: &'p Expr, r: &'p Expr, ty: &Ty, env: &mut Env) -> String {
        match op {
            BinOp::And => {
                let a = self.eval(l, env);
                self.path.push(a.clone());
                let b = self.eval(r, env);
                self.path.pop();
                return self.def(ty, format!("(and {a} {b})"));
            }
            BinOp::Or => {
                let a = self.eval(l, env);
                self.path.push(format!("(not {a})"));
                let b = self.eval(r, env);
                self.path.pop();
                return self.def(ty, format!("(or {a} {b})"));
            }
            _ => {}
        }
        let a = self.eval(l, env);
        let b = self.eval(r, env);
        let operand = self.ty(&l.ty);
        let term = match op {
            BinOp::Eq => format!("(= {a} {b})"),
            BinOp::Ne => format!("(not (= {a} {b}))"),
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => self.compare(op, &operand, &a, &b),
            BinOp::Concat => format!("(sconcat {a} {b})"),
            BinOp::Add | BinOp::Sub | BinOp::Mul => {
                if operand == Ty::Float {
                    let f = match op {
                        BinOp::Add => "fadd",
                        BinOp::Sub => "fsub",
                        _ => "fmul",
                    };
                    format!("({f} {a} {b})")
                } else {
                    let f = match op {
                        BinOp::Add => "+",
                        BinOp::Sub => "-",
                        _ => "*",
                    };
                    format!("({f} {a} {b})")
                }
            }
            BinOp::Div | BinOp::Rem => {
                let text = self.src_text(r.span, false);
                if operand == Ty::Float {
                    let zero = self.enc.float_lit(0.0);
                    self.obligation(
                        Kind::FloatDomain,
                        r.span,
                        "the divisor may be 0.0".into(),
                        Some(format!("`{text} != 0.0`")),
                        format!("(not (= {b} {zero}))"),
                    );
                    format!("(fdiv {a} {b})")
                } else {
                    self.ensure_div();
                    self.obligation(
                        Kind::DivZero,
                        r.span,
                        "the divisor may be 0".into(),
                        Some(format!("`{text} != 0`")),
                        format!("(not (= {b} 0))"),
                    );
                    format!(
                        "({} {a} {b})",
                        if op == BinOp::Div { "tdiv" } else { "trem" }
                    )
                }
            }
            BinOp::And | BinOp::Or => String::new(),
        };
        self.def(ty, term)
    }

    fn compare(&mut self, op: BinOp, operand: &Ty, a: &str, b: &str) -> String {
        match operand {
            Ty::Float => match op {
                BinOp::Lt => format!("(flt {a} {b})"),
                BinOp::Le => format!("(fle {a} {b})"),
                BinOp::Gt => format!("(flt {b} {a})"),
                _ => format!("(fle {b} {a})"),
            },
            Ty::Str => match op {
                BinOp::Lt => format!("(slt {a} {b})"),
                BinOp::Le => format!("(sle {a} {b})"),
                BinOp::Gt => format!("(slt {b} {a})"),
                _ => format!("(sle {b} {a})"),
            },
            _ => {
                let o = match op {
                    BinOp::Lt => "<",
                    BinOp::Le => "<=",
                    BinOp::Gt => ">",
                    _ => ">=",
                };
                format!("({o} {a} {b})")
            }
        }
    }

    // ------------------------------------------------------------ constructors and refinements

    fn ctor_term(&mut self, adt: &Ty, variant: usize, terms: &[String], args: &[Expr]) -> String {
        let exprs: Vec<Option<&Expr>> = args.iter().map(Some).collect();
        self.ctor_term_with(adt, variant, terms, &exprs)
    }

    #[allow(clippy::needless_range_loop)]
    fn ctor_term_with(
        &mut self,
        adt: &Ty,
        variant: usize,
        terms: &[String],
        exprs: &[Option<&Expr>],
    ) -> String {
        let adt = self.ty(adt);
        if let Ty::Adt(tid, targs) = &adt {
            let def = &self.prog.types[*tid];
            let map: Vec<(String, Ty)> = def
                .params
                .iter()
                .cloned()
                .zip(targs.iter().cloned())
                .collect();
            if let Some(fields) = def.variant_fields(variant) {
                for (j, f) in fields.iter().enumerate() {
                    if f.refine.is_empty() {
                        continue;
                    }
                    let Some(Some(ex)) = exprs.get(j) else {
                        continue;
                    };
                    let saved = std::mem::replace(&mut self.tmap, map.clone());
                    for r in &f.refine {
                        let goal = self.eval_type_refine(r, &terms[j]);
                        self.tmap = saved.clone();
                        self.obligation(
                            Kind::Refinement,
                            ex.span,
                            format!(
                                "this value may not satisfy the refinement of field `{}`",
                                f.name
                            ),
                            None,
                            goal,
                        );
                        self.tmap.clone_from(&map);
                    }
                    self.tmap = saved;
                }
            }
        }
        let c = self.enc.ctor(&adt, variant);
        let t = if terms.is_empty() {
            c
        } else {
            format!("({c} {})", terms.join(" "))
        };
        self.def(&adt, t)
    }

    /// Evaluates a type-level refinement (own frame) for `subject`, without emitting obligations.
    fn eval_type_refine(&mut self, r: &'p Refinement, subject: &str) -> String {
        let saved = std::mem::replace(&mut self.emit, false);
        let mut env: Env = vec![String::new(); r.nlocals.max(r.it + 1) as usize];
        env[r.it as usize] = subject.to_string();
        let t = self.eval(&r.pred, &mut env);
        self.emit = saved;
        t
    }

    /// Assumes the refinements of a field that was just read or destructured.
    fn assume_field_refinements(
        &mut self,
        adt: &Ty,
        variant: usize,
        field: usize,
        value: &str,
        guard: Option<&str>,
    ) {
        let Ty::Adt(tid, targs) = adt else { return };
        let def = &self.prog.types[*tid];
        let map: Vec<(String, Ty)> = def
            .params
            .iter()
            .cloned()
            .zip(targs.iter().cloned())
            .collect();
        let Some(f) = def.variant_fields(variant).and_then(|fs| fs.get(field)) else {
            return;
        };
        for r in &f.refine {
            let saved = std::mem::replace(&mut self.tmap, map.clone());
            let t = self.eval_type_refine(r, value);
            self.tmap = saved;
            match guard {
                Some(g) => self.fact(format!("(=> {g} {t})")),
                None => self.fact(t),
            }
        }
    }

    // ------------------------------------------------------------ patterns and match

    fn pat_cond(&mut self, p: &Pat, v: &str, env: &mut Env) -> String {
        let pty = self.ty(&p.ty);
        match &p.kind {
            PatKind::Wild | PatKind::Unit => "true".into(),
            PatKind::Bind(slot) => {
                env[*slot as usize] = v.to_string();
                "true".into()
            }
            PatKind::Lit(l) => match l {
                LitVal::Bool(true) => v.to_string(),
                LitVal::Bool(false) => format!("(not {v})"),
                LitVal::Int(n) => format!("(= {v} {})", int_lit(n)),
                LitVal::Float(f) => format!("(= {v} {})", self.enc.float_lit(*f)),
                LitVal::Str(s) => format!("(= {v} {})", self.enc.str_lit(s)),
            },
            PatKind::Ctor { variant, args, .. } => {
                let ctor = self.enc.ctor(&pty, *variant);
                let tester = format!("((_ is {ctor}) {v})");
                let mut conds = vec![tester.clone()];
                for (j, a) in args.iter().enumerate() {
                    let sel = self.enc.selector(&pty, *variant, j);
                    let fv = self.def(&a.ty, format!("({sel} {v})"));
                    self.assume_field_refinements(&pty, *variant, j, &fv, Some(&tester));
                    let c = self.pat_cond(a, &fv, env);
                    if c != "true" {
                        conds.push(c);
                    }
                }
                and_all(&conds)
            }
            PatKind::Tuple(ps) => {
                let mut conds = Vec::new();
                for (j, a) in ps.iter().enumerate() {
                    let sel = self.enc.selector(&pty, 0, j);
                    let fv = self.def(&a.ty, format!("({sel} {v})"));
                    let c = self.pat_cond(a, &fv, env);
                    if c != "true" {
                        conds.push(c);
                    }
                }
                and_all(&conds)
            }
            PatKind::List { items, rest } => {
                let ops = self.enc.list_ops(&pty);
                let n = items.len();
                let len = format!("({} {v})", ops.len);
                let mut conds = vec![if rest.is_some() {
                    format!("(>= {len} {n})")
                } else {
                    format!("(= {len} {n})")
                }];
                for (j, a) in items.iter().enumerate() {
                    let fv = self.def(&a.ty, format!("({} {v} {j})", ops.get));
                    let c = self.pat_cond(a, &fv, env);
                    if c != "true" {
                        conds.push(c);
                    }
                }
                if let Some(Some(slot)) = rest {
                    let tail = self.def(&pty, format!("({} {v} {n})", ops.drop));
                    env[*slot as usize] = tail;
                }
                and_all(&conds)
            }
        }
    }

    fn match_expr(
        &mut self,
        scrut: &'p Expr,
        arms: &'p [tez_types::hir::Arm],
        ty: &Ty,
        env: &mut Env,
    ) -> String {
        let s = self.eval(scrut, env);
        let mut results: Vec<(String, String)> = Vec::new();
        let base_path = self.path.len();
        for arm in arms {
            let mark = self.watch.len();
            let c = self.pat_cond(&arm.pat, &s, env);
            self.path.push(c.clone());
            let mut full = vec![c];
            if let Some(g) = &arm.guard {
                let gv = self.eval(g, env);
                self.path.push(gv.clone());
                full.push(gv);
            }
            let v = self.eval(&arm.body, env);
            self.path.truncate(base_path);
            self.watch.truncate(mark);
            let cond = and_all(&full);
            // later arms are reached only when this one did not match
            self.path.push(format!("(not {cond})"));
            results.push((cond, v));
        }
        self.path.truncate(base_path);
        let mut it = results.into_iter().rev();
        let Some((_, mut acc)) = it.next() else {
            return "0".into();
        };
        for (c, v) in it {
            acc = format!("(ite {c} {v} {acc})");
        }
        self.def(ty, acc)
    }

    // ------------------------------------------------------------ calls

    fn call_value(&mut self, callee: &'p Expr, args: &'p [Expr], ty: &Ty, env: &mut Env) -> String {
        if let ExprKind::Lambda { params, body } = &callee.kind {
            // immediately applied lambda: evaluate the body with the actual arguments
            let terms: Vec<String> = args.iter().map(|a| self.eval(a, env)).collect();
            for (slot, t) in params.iter().zip(terms) {
                env[*slot as usize] = t;
            }
            return self.eval(body, env);
        }
        let c = self.eval(callee, env);
        let terms: Vec<String> = args.iter().map(|a| self.eval(a, env)).collect();
        if let Some(r) = self.apply_lam(&c, terms.clone()) {
            return r;
        }
        for t in &terms {
            self.escape(t);
        }
        let fty = self.ty(&callee.ty);
        let ap = self.enc.ap_fn(&fty);
        self.def(ty, format!("({ap} {c} {})", terms.join(" ")))
    }

    fn fn_value(&mut self, f: FnId, targs: &[Ty], ty: &Ty, span: ByteSpan) -> String {
        let prog = self.prog;
        let def = &prog.fns[f];
        if let Some(cur) = self.cur_fn
            && def.is_recursive
            && self.same_scc(cur, f)
        {
            self.obligation(
                Kind::Termination,
                span,
                format!("`{}` is recursive and cannot be used as a value", def.name),
                Some("call it directly with a decreasing argument".into()),
                "false".into(),
            );
        }
        if !def.requires.is_empty() || def.params.iter().any(|p| !p.refine.is_empty()) {
            // calling through a value skips the call-site checks, so the contract must be trivial
            let tm: Vec<(String, Ty)> = def
                .tparams
                .iter()
                .cloned()
                .zip(targs.iter().map(|t| self.ty(t)))
                .collect();
            let saved_tm = std::mem::replace(&mut self.tmap, tm);
            let saved_emit = std::mem::replace(&mut self.emit, false);
            let mut env: Env = vec![String::new(); def.nlocals as usize];
            for p in &def.params {
                let s = self.sort(&p.ty);
                env[p.local as usize] = self.enc.declare_const("fa", &s);
            }
            let mut goals = Vec::new();
            for p in &def.params {
                for r in &p.refine {
                    let subject = env[p.local as usize].clone();
                    env[r.it as usize] = subject;
                    goals.push(self.eval(&r.pred, &mut env));
                }
            }
            for r in &def.requires {
                goals.push(self.eval(r, &mut env));
            }
            self.emit = saved_emit;
            self.tmap = saved_tm;
            self.obligation(
                Kind::Precondition,
                span,
                format!(
                    "`{}` has a contract, so it cannot be used as a value",
                    def.name
                ),
                Some("a lambda that checks the condition first".into()),
                and_all(&goals),
            );
        }
        let s = self.sort(ty);
        let key = format!("fnv|{f}|{s}");
        self.enc.fn_sym(&key, &[], &s)
    }

    fn call(
        &mut self,
        f: FnId,
        targs: &[Ty],
        args: &'p [Expr],
        ret_ty: &Ty,
        span: ByteSpan,
        env: &mut Env,
    ) -> String {
        let prog = self.prog;
        let def = &prog.fns[f];
        let hof = if def.is_extern {
            hof_info(&def.full_name())
        } else {
            None
        };
        let inlinable = self.can_inline(f);
        let mut terms: Vec<String> = Vec::with_capacity(args.len());
        for (k, a) in args.iter().enumerate() {
            let t = match (&a.kind, hof) {
                (ExprKind::Lambda { .. }, Some((li, ep))) if k == li && !terms.is_empty() => {
                    let list_ty = self.ty(&args[0].ty);
                    let list = terms[0].clone();
                    self.lambda_with_elem(a, env, &list, &list_ty, ep)
                }
                (ExprKind::FnRef(g, gt), Some((li, ep))) if k == li && !terms.is_empty() => {
                    let list_ty = self.ty(&args[0].ty);
                    let list = terms[0].clone();
                    self.fnref_with_elem(*g, gt, a, &list, &list_ty, ep)
                }
                (ExprKind::Lambda { .. }, _) if inlinable && !self.closed => {
                    self.lambda_deferred(a, env)
                }
                _ => self.eval(a, env),
            };
            terms.push(t);
        }
        let arg_tys: Vec<Ty> = args.iter().map(|a| self.ty(&a.ty)).collect();
        let spans: Vec<ByteSpan> = args.iter().map(|a| a.span).collect();
        self.call_terms(f, targs, terms, arg_tys, spans, ret_ty, span, inlinable)
    }

    #[allow(clippy::too_many_arguments)]
    fn call_terms(
        &mut self,
        f: FnId,
        targs: &[Ty],
        terms: Vec<String>,
        arg_tys: Vec<Ty>,
        arg_spans: Vec<ByteSpan>,
        ret_ty: &Ty,
        span: ByteSpan,
        inlinable: bool,
    ) -> String {
        let prog = self.prog;
        let def = &prog.fns[f];
        let targs_now: Vec<Ty> = targs.iter().map(|t| self.ty(t)).collect();
        let callee_map: Vec<(String, Ty)> = def.tparams.iter().cloned().zip(targs_now).collect();
        let ret_now = self.ty(ret_ty);

        // callee frame with parameters bound to the arguments
        let mut cenv: Env = vec![String::new(); def.nlocals as usize];
        for (p, t) in def.params.iter().zip(&terms) {
            cenv[p.local as usize].clone_from(t);
        }
        let outer_emit = self.emit;
        let outer_map = std::mem::replace(&mut self.tmap, callee_map);
        let outer_watch = std::mem::replace(&mut self.watch_on, false);

        // obligations: refined parameter types, then `requires`
        for (p, a) in def.params.iter().zip(&arg_spans) {
            for r in &p.refine {
                let subject = cenv[p.local as usize].clone();
                cenv[r.it as usize] = subject;
                self.emit = false;
                let goal = self.eval(&r.pred, &mut cenv);
                self.emit = outer_emit;
                let kind = if def.is_prelude && def.full_name() == "Float.sqrt" {
                    Kind::FloatDomain
                } else {
                    Kind::Refinement
                };
                let msg = if kind == Kind::FloatDomain {
                    "the argument of `sqrt` may be negative".to_string()
                } else {
                    format!(
                        "this argument may not satisfy the refined parameter `{}` of `{}`",
                        p.name, def.name
                    )
                };
                let text = self.src_text(r.pred.span, def.is_prelude);
                self.obligation(
                    kind,
                    *a,
                    msg,
                    (!text.is_empty()).then(|| format!("`where {text}`")),
                    goal,
                );
            }
        }
        for r in &def.requires {
            self.emit = false;
            let goal = self.eval(r, &mut cenv);
            self.emit = outer_emit;
            let text = self.src_text(r.span, def.is_prelude);
            let msg = if def.is_prelude && def.full_name() == "List.get" {
                "the index may be out of range".to_string()
            } else {
                format!("this call may violate the precondition of `{}`", def.name)
            };
            let kind = if def.is_prelude && def.full_name() == "List.get" {
                Kind::IndexBounds
            } else {
                Kind::Precondition
            };
            self.obligation(
                kind,
                span,
                msg,
                (!text.is_empty()).then(|| format!("`requires {text}`")),
                goal,
            );
        }

        // termination: recursive calls within the current function's cycle
        if let (Some(cur), Some(m_cur)) = (self.cur_fn, self.cur_measure.clone())
            && def.is_recursive
            && self.same_scc(cur, f)
            && let Some(d) = &def.decreases
        {
            self.emit = false;
            let m_new = self.measure(d, &mut cenv);
            self.emit = outer_emit;
            self.obligation(
                Kind::Termination,
                span,
                format!(
                    "the `decreases` measure of `{}` may be negative here",
                    def.name
                ),
                Some("a non-negative measure".into()),
                format!("(>= {m_new} 0)"),
            );
            self.obligation(
                Kind::Termination,
                span,
                format!(
                    "the `decreases` measure may not decrease in this call to `{}`",
                    def.name
                ),
                Some("a strictly smaller measure".into()),
                format!("(< {m_new} {m_cur})"),
            );
        }

        if !inlinable {
            for t in &terms {
                self.escape(t);
            }
        }

        // the result: builtin model, inlined body, or contract
        let result = if let Some(r) = self.builtin(
            def.full_name().as_str(),
            def.is_extern,
            &terms,
            &arg_tys,
            &ret_now,
        ) {
            r
        } else if inlinable {
            self.inline_budget -= 1;
            self.inline_depth += 1;
            self.emit = false;
            let r = match &def.body {
                Some(b) => self.eval(b, &mut cenv),
                None => "0".into(),
            };
            self.emit = outer_emit;
            self.inline_depth -= 1;
            r
        } else {
            let param_sorts: Vec<String> = arg_tys.iter().map(|t| self.enc.sort(t)).collect();
            let ret_sort = self.enc.sort(&ret_now);
            let key = format!("{f}|{}|{ret_sort}", param_sorts.join(","));
            let (sym, fresh) = self.enc.fn_sym_new(&key, &param_sorts, &ret_sort);
            if fresh && !def.is_extern {
                self.define_by_axiom(f, &sym, &param_sorts);
            }
            let t = if terms.is_empty() {
                sym.clone()
            } else {
                format!("({sym} {})", terms.join(" "))
            };
            let r = self.def_concrete(&ret_now, t);
            // assume the contract's consequences on this path
            self.emit = false;
            cenv[def.result_local as usize].clone_from(&r);
            for e in &def.ensures {
                let t = self.eval(e, &mut cenv);
                self.guarded_fact(t);
            }
            for rr in &def.ret_refine {
                cenv[rr.it as usize].clone_from(&r);
                let t = self.eval(&rr.pred, &mut cenv);
                self.guarded_fact(t);
            }
            self.emit = outer_emit;
            r
        };
        self.tmap = outer_map;
        self.watch_on = outer_watch;
        result
    }

    // ------------------------------------------------------------ lambdas

    fn register_lam(&mut self, e: &'p Expr, env: &Env, state: LamState) -> String {
        let lam_ty = self.ty(&e.ty);
        let s = self.enc.sort(&lam_ty);
        let lam = self.enc.declare_const("lam", &s);
        self.lams.insert(
            lam.clone(),
            LamInfo {
                expr: e,
                env: env.clone(),
                tmap: self.tmap.clone(),
                path: self.path.clone(),
                state,
                applied: false,
                emit: self.emit,
            },
        );
        lam
    }

    /// A lambda in an ordinary position: checked for arbitrary arguments right away.
    fn lambda_generic(&mut self, e: &'p Expr, env: &mut Env) -> String {
        let lam = self.register_lam(e, env, LamState::Verified);
        self.check_lam_body(e, env, None);
        lam
    }

    /// A lambda handed to an inlined callee: checked where (and if) it is applied.
    fn lambda_deferred(&mut self, e: &'p Expr, env: &mut Env) -> String {
        self.register_lam(e, env, LamState::Deferred)
    }

    /// Checks a lambda body with fresh arguments. `elem` = `(list, list type, parameter index)`:
    /// that parameter is known to be an element of the list.
    fn check_lam_body(&mut self, e: &'p Expr, env: &mut Env, elem: Option<(&str, &Ty, usize)>) {
        let ExprKind::Lambda { params, body } = &e.kind else {
            return;
        };
        let lam_ty = self.ty(&e.ty);
        let saved_watch = std::mem::replace(&mut self.watch_on, false);
        let pre = self.path.len();
        let mut consts = Vec::new();
        if let Ty::Fn(ps, _) = &lam_ty {
            for (slot, pt) in params.iter().zip(ps) {
                let s = self.enc.sort(pt);
                let c = self.enc.declare_const("a", &s);
                env[*slot as usize].clone_from(&c);
                consts.push(c);
            }
        }
        if let Some((list, list_ty, ep)) = elem
            && let Some(a) = consts.get(ep)
        {
            let ops = self.enc.list_ops(list_ty);
            let idx = self.enc.declare_const("idx", "Int");
            self.path.push(format!(
                "(and (<= 0 {idx}) (< {idx} ({} {list})) (= {a} ({} {list} {idx})))",
                ops.len, ops.get
            ));
        }
        let _ = self.eval(body, env);
        self.path.truncate(pre);
        self.watch_on = saved_watch;
    }

    fn lambda_with_elem(
        &mut self,
        e: &'p Expr,
        env: &mut Env,
        list: &str,
        list_ty: &Ty,
        ep: usize,
    ) -> String {
        let lam = self.register_lam(e, env, LamState::Verified);
        self.check_lam_body(e, env, Some((list, list_ty, ep)));
        lam
    }

    /// `xs.map(g)` for a named function `g`: checks `g`'s contract for an arbitrary element.
    fn fnref_with_elem(
        &mut self,
        g: FnId,
        gt: &[Ty],
        e: &'p Expr,
        list: &str,
        list_ty: &Ty,
        ep: usize,
    ) -> String {
        let prog = self.prog;
        let def = &prog.fns[g];
        let targs: Vec<Ty> = gt.iter().map(|t| self.ty(t)).collect();
        let map: Vec<(String, Ty)> = def
            .tparams
            .iter()
            .cloned()
            .zip(targs.iter().cloned())
            .collect();
        let ptys: Vec<Ty> = def.params.iter().map(|p| p.ty.subst_params(&map)).collect();
        let ret = def.ret.subst_params(&map);
        let consts: Vec<String> = ptys
            .iter()
            .map(|t| {
                let s = self.enc.sort(t);
                self.enc.declare_const("a", &s)
            })
            .collect();
        let pre = self.path.len();
        if let Some(a) = consts.get(ep) {
            let ops = self.enc.list_ops(list_ty);
            let idx = self.enc.declare_const("idx", "Int");
            self.path.push(format!(
                "(and (<= 0 {idx}) (< {idx} ({} {list})) (= {a} ({} {list} {idx})))",
                ops.len, ops.get
            ));
        }
        let spans = vec![e.span; consts.len()];
        let _ = self.call_terms(g, &targs, consts, ptys, spans, &ret, e.span, false);
        self.path.truncate(pre);
        let fty = self.ty(&e.ty);
        let s = self.enc.sort(&fty);
        self.enc.fn_sym(&format!("fnv|{g}|{s}"), &[], &s)
    }

    /// Applies a known lambda by evaluating its body with the actual arguments.
    fn apply_lam(&mut self, lam: &str, args: Vec<String>) -> Option<String> {
        let info = self.lams.get_mut(lam)?;
        info.applied = true;
        let (expr, mut env2, tmap, state, emit_at) = (
            info.expr,
            info.env.clone(),
            info.tmap.clone(),
            info.state,
            info.emit,
        );
        let ExprKind::Lambda { params, body } = &expr.kind else {
            return None;
        };
        for (slot, t) in params.iter().zip(args) {
            env2[*slot as usize] = t;
        }
        let saved_tmap = std::mem::replace(&mut self.tmap, tmap);
        // a body already checked for all arguments needs no obligations here
        let saved_emit = std::mem::replace(&mut self.emit, state == LamState::Deferred && emit_at);
        let r = self.eval(body, &mut env2);
        self.emit = saved_emit;
        self.tmap = saved_tmap;
        Some(r)
    }

    /// A lambda is about to leave the evaluator's sight: check its body for all arguments now.
    fn escape(&mut self, term: &str) {
        let Some(info) = self.lams.get_mut(term) else {
            return;
        };
        if info.state != LamState::Deferred {
            return;
        }
        info.state = LamState::Verified;
        let (expr, mut env2, tmap, path, emit_at) = (
            info.expr,
            info.env.clone(),
            info.tmap.clone(),
            info.path.clone(),
            info.emit,
        );
        let saved = (
            std::mem::replace(&mut self.tmap, tmap),
            std::mem::replace(&mut self.path, path),
            std::mem::replace(&mut self.emit, emit_at),
        );
        self.check_lam_body(expr, &mut env2, None);
        self.tmap = saved.0;
        self.path = saved.1;
        self.emit = saved.2;
    }

    // ------------------------------------------------------------ definitions of recursive functions

    /// Asserts `requires(args) => f(args) = body(args)` for a function whose body can be written
    /// as a closed term. Sound because termination is verified separately (the equation then has
    /// exactly one solution) and the guard keeps it from constraining inputs outside the contract.
    fn define_by_axiom(&mut self, f: FnId, sym: &str, param_sorts: &[String]) {
        let prog = self.prog;
        let def = &prog.fns[f];
        let Some(body) = &def.body else { return };
        let saved = (
            self.closed,
            self.closed_fail,
            self.emit,
            std::mem::take(&mut self.path),
            self.watch_on,
            self.cur_fn,
        );
        self.closed = true;
        self.closed_fail = false;
        self.emit = false;
        self.watch_on = false;
        let mut env: Env = vec![String::new(); def.nlocals as usize];
        let vars: Vec<String> = (0..def.params.len())
            .map(|i| format!("x{}", i + 1))
            .collect();
        for (p, v) in def.params.iter().zip(&vars) {
            env[p.local as usize].clone_from(v);
        }
        let mut guards = Vec::new();
        for p in &def.params {
            for r in &p.refine {
                let subject = env[p.local as usize].clone();
                env[r.it as usize] = subject;
                guards.push(self.eval(&r.pred, &mut env));
            }
        }
        for r in &def.requires {
            guards.push(self.eval(r, &mut env));
        }
        let term = self.eval(body, &mut env);
        let ok = !self.closed_fail;
        self.closed = saved.0;
        self.closed_fail = saved.1;
        self.emit = saved.2;
        self.path = saved.3;
        self.watch_on = saved.4;
        self.cur_fn = saved.5;
        if !ok || vars.is_empty() {
            return;
        }
        let binders: Vec<(String, String)> = vars
            .iter()
            .cloned()
            .zip(param_sorts.iter().cloned())
            .collect();
        let lhs = format!("({sym} {})", vars.join(" "));
        let flag = if def.is_recursive {
            Some(self.term_flag(f))
        } else {
            None
        };
        let equation = format!("(=> {} (= {lhs} {term}))", and_all(&guards));
        // a recursive definition is only assumed once the function is proved to terminate:
        // the equation of a diverging function (`f(x) = f(x) + 1`) would be contradictory
        let body = match &flag {
            Some(fl) => format!("(=> {fl} {equation})"),
            None => equation,
        };
        let axiom = Enc::quant_owned(&binders, &lhs, &body);
        self.enc.add_axiom(axiom);
    }

    fn can_inline(&self, f: FnId) -> bool {
        let d = &self.prog.fns[f];
        !d.is_extern
            && !d.is_recursive
            && d.body.is_some()
            && self.inline_depth < 5
            && self.inline_budget > 0
    }

    fn builtin(
        &mut self,
        name: &str,
        is_extern: bool,
        a: &[String],
        tys: &[Ty],
        ret: &Ty,
    ) -> Option<String> {
        if !is_extern {
            return None;
        }
        let t = match name {
            "List.len" => {
                let ops = self.enc.list_ops(tys.first()?);
                format!("({} {})", ops.len, a.first()?)
            }
            "List.get" => {
                let ops = self.enc.list_ops(tys.first()?);
                format!("({} {} {})", ops.get, a.first()?, a.get(1)?)
            }
            "List.append" => {
                let ops = self.enc.list_ops(tys.first()?);
                format!("({} {} {})", ops.snoc, a.first()?, a.get(1)?)
            }
            "List.concat" => {
                let ops = self.enc.list_ops(tys.first()?);
                format!("({} {} {})", ops.concat, a.first()?, a.get(1)?)
            }
            "List.take" => {
                let ops = self.enc.list_ops(tys.first()?);
                format!("({} {} {})", ops.take, a.first()?, a.get(1)?)
            }
            "List.drop" => {
                let ops = self.enc.list_ops(tys.first()?);
                format!("({} {} {})", ops.drop, a.first()?, a.get(1)?)
            }
            "List.reverse" => {
                let ops = self.enc.list_ops(tys.first()?);
                format!("({} {})", ops.rev, a.first()?)
            }
            "List.range" => {
                let ops = self.enc.list_ops(ret);
                format!("({} {} {})", ops.range?, a.first()?, a.get(1)?)
            }
            "List.repeat" => {
                let ops = self.enc.list_ops(ret);
                format!("({} {} {})", ops.rep, a.first()?, a.get(1)?)
            }
            "Str.len" => format!("(slen {})", a.first()?),
            _ => return None,
        };
        Some(self.def_concrete(ret, t))
    }

    /// Locals of the function being verified (for diagnostics).
    #[allow(dead_code)]
    pub fn locals(&self, f: FnId) -> &[LocalInfo] {
        &self.prog.fns[f].locals
    }
}
