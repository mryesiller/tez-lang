//! Expression inference and checking.

use num_bigint::BigInt;
use tez_diag::Code;
use tez_syntax::ast::{self, BinOp, ExprKind as A, Ident, UnOp};
use tez_syntax::{ByteSpan, Problem, to_lower_camel};

use super::{Checker, Pending, suggest};
use crate::hir::{self, Expr, ExprKind as H, LocalId};
use crate::ty::Ty;

pub(crate) enum UErr {
    Mismatch,
    Occurs,
}

fn mk(kind: H, ty: Ty, span: ByteSpan) -> Expr {
    Expr { kind, ty, span }
}

fn error_expr(span: ByteSpan) -> Expr {
    mk(H::Unit, Ty::Error, span)
}

impl<'a> Checker<'a> {
    // ------------------------------------------------------------ unification

    pub fn shallow(&self, t: &Ty) -> Ty {
        let mut t = t.clone();
        while let Ty::Var(i) = t {
            match self.subst.get(i as usize).and_then(Option::as_ref) {
                Some(n) => t = n.clone(),
                None => break,
            }
        }
        t
    }

    pub fn resolve(&self, t: &Ty) -> Ty {
        t.map(&mut |x| match x {
            Ty::Var(i) => Some(match self.subst.get(*i as usize).and_then(Option::as_ref) {
                Some(n) => self.resolve(n),
                None => x.clone(),
            }),
            _ => None,
        })
    }

    fn occurs(&self, v: u32, t: &Ty) -> bool {
        match self.shallow(t) {
            Ty::Var(i) => i == v,
            Ty::List(x) => self.occurs(v, &x),
            Ty::Tuple(ts) | Ty::Adt(_, ts) => ts.iter().any(|x| self.occurs(v, x)),
            Ty::Fn(ps, r) => ps.iter().any(|x| self.occurs(v, x)) || self.occurs(v, &r),
            _ => false,
        }
    }

    pub fn unify(&mut self, a: &Ty, b: &Ty) -> Result<(), UErr> {
        let a = self.shallow(a);
        let b = self.shallow(b);
        match (&a, &b) {
            (Ty::Error, _) | (_, Ty::Error) => Ok(()),
            (Ty::Var(i), Ty::Var(j)) if i == j => Ok(()),
            (Ty::Var(i), t) | (t, Ty::Var(i)) => {
                if self.occurs(*i, t) {
                    return Err(UErr::Occurs);
                }
                self.subst[*i as usize] = Some(t.clone());
                Ok(())
            }
            (Ty::Int, Ty::Int)
            | (Ty::Float, Ty::Float)
            | (Ty::Bool, Ty::Bool)
            | (Ty::Str, Ty::Str)
            | (Ty::Unit, Ty::Unit) => Ok(()),
            (Ty::List(x), Ty::List(y)) => self.unify(x, y),
            (Ty::Tuple(xs), Ty::Tuple(ys)) if xs.len() == ys.len() => {
                for (x, y) in xs.iter().zip(ys) {
                    self.unify(x, y)?;
                }
                Ok(())
            }
            (Ty::Fn(p, r), Ty::Fn(q, s)) if p.len() == q.len() => {
                for (x, y) in p.iter().zip(q) {
                    self.unify(x, y)?;
                }
                self.unify(r, s)
            }
            (Ty::Adt(i, xs), Ty::Adt(j, ys)) if i == j && xs.len() == ys.len() => {
                for (x, y) in xs.iter().zip(ys) {
                    self.unify(x, y)?;
                }
                Ok(())
            }
            (Ty::Param(m), Ty::Param(n)) if m == n => Ok(()),
            _ => Err(UErr::Mismatch),
        }
    }

    /// Unifies, reporting a mismatch at `span`. Returns false on failure.
    pub fn expect_ty(&mut self, span: ByteSpan, expected: &Ty, found: &Ty) -> bool {
        match self.unify(expected, found) {
            Ok(()) => true,
            Err(UErr::Occurs) => {
                self.err(Problem::new(
                    Code::TypeInfinite,
                    "this would make a type contain itself",
                    span,
                ));
                false
            }
            Err(UErr::Mismatch) => {
                let (e, f) = (self.show(expected), self.show(found));
                let mut p = Problem::new(
                    Code::TypeMismatch,
                    format!("expected `{e}`, found `{f}`"),
                    span,
                )
                .expected(format!("`{e}`"));
                if e == "Float" && f == "Int" {
                    p.message.push_str(" (use `.toFloat()`)");
                }
                self.err(p);
                false
            }
        }
    }

    // ------------------------------------------------------------ helpers

    fn text(&self, span: ByteSpan) -> &'a str {
        span.text(self.src)
    }

    fn sig_string(&self, f: usize) -> String {
        let d = &self.program.fns[f];
        let ps: Vec<String> = d
            .params
            .iter()
            .map(|p| self.program.show_ty(&p.ty))
            .collect();
        format!(
            "{}({}) -> {}",
            d.full_name(),
            ps.join(", "),
            self.program.show_ty(&d.ret)
        )
    }

    /// Fresh instantiation of a function's signature.
    fn instantiate(&mut self, f: usize) -> (Vec<Ty>, Ty, Vec<Ty>) {
        let (tparams, params, ret) = {
            let d = &self.program.fns[f];
            (
                d.tparams.clone(),
                d.params.iter().map(|p| p.ty.clone()).collect::<Vec<_>>(),
                d.ret.clone(),
            )
        };
        let vars: Vec<Ty> = tparams.iter().map(|_| self.fresh()).collect();
        let map: Vec<(String, Ty)> = tparams.into_iter().zip(vars.iter().cloned()).collect();
        let ptys = params.iter().map(|t| t.subst_params(&map)).collect();
        (ptys, ret.subst_params(&map), vars)
    }

    fn note_call(&mut self, f: usize, span: ByteSpan) {
        if !self.program.fns[f].is_prelude {
            self.cur_calls.push(f);
            if self.in_const.is_some() {
                self.err(
                    Problem::new(
                        Code::ConstInit,
                        "a constant cannot call user functions",
                        span,
                    )
                    .expected("literals, constructors, earlier constants and prelude functions"),
                );
            }
        }
    }

    fn unknown_name(&mut self, id: &Ident) -> Expr {
        let n = id.name.as_str();
        if n == "it" || n == "result" {
            let where_ = if n == "it" {
                "a `where` refinement"
            } else {
                "an `ensures` clause"
            };
            self.err(Problem::new(
                Code::ContractScope,
                format!("`{n}` is only available inside {where_}"),
                id.span,
            ));
            return error_expr(id.span);
        }
        let mut cands: Vec<String> = Vec::new();
        for s in &self.scopes {
            cands.extend(s.iter().map(|(n, _, _)| n.clone()));
        }
        cands.extend(self.fn_names.keys().cloned());
        cands.extend(self.const_names.keys().cloned());
        let camel = to_lower_camel(n);
        let hint = if camel != n && cands.contains(&camel) {
            Some(camel)
        } else {
            suggest(n, cands.iter().map(String::as_str))
        };
        let mut p = Problem::new(Code::NameUnknown, format!("unknown name `{n}`"), id.span);
        if let Some(h) = hint {
            p = p
                .expected(format!("`{h}`"))
                .fix(format!("use `{h}`"), id.span, h);
        }
        self.err(p);
        error_expr(id.span)
    }

    // ------------------------------------------------------------ expression entry

    pub fn expr(&mut self, e: &'a ast::Expr, exp: Option<&Ty>) -> Expr {
        let out = self.expr_kind(e, exp);
        if let Some(x) = exp {
            self.expect_ty(e.span, x, &out.ty);
        }
        out
    }

    fn expr_kind(&mut self, e: &'a ast::Expr, exp: Option<&Ty>) -> Expr {
        let span = e.span;
        match &e.kind {
            A::Int(s) => {
                let v = s.parse::<BigInt>().unwrap_or_default();
                mk(H::Int(v), Ty::Int, span)
            }
            A::Float(s) => {
                let v = s.parse::<f64>().unwrap_or(f64::INFINITY);
                if !v.is_finite() {
                    self.err(
                        Problem::new(Code::LexBadNumber, "this Float literal is too large", span)
                            .expected("a literal below 1.8e308"),
                    );
                }
                mk(H::Float(v), Ty::Float, span)
            }
            A::Str(s) => mk(H::Str(s.clone()), Ty::Str, span),
            A::Bool(b) => mk(H::Bool(*b), Ty::Bool, span),
            A::Unit => mk(H::Unit, Ty::Unit, span),
            A::Paren(inner) => {
                let mut x = self.expr(inner, exp);
                x.span = span;
                x
            }
            A::Var(id) => self.var(id, span),
            A::Ctor(id) => self.ctor_value(id, span),
            A::Path { module, name } => self.path_value(module, name, span),
            A::List(items) => {
                let elem = match exp.map(|t| self.shallow(t)) {
                    Some(Ty::List(t)) => *t,
                    _ => self.fresh(),
                };
                let xs: Vec<Expr> = items.iter().map(|i| self.expr(i, Some(&elem))).collect();
                mk(H::List(xs), Ty::list(elem), span)
            }
            A::Tuple(items) => {
                let hints: Vec<Option<Ty>> = match exp.map(|t| self.shallow(t)) {
                    Some(Ty::Tuple(ts)) if ts.len() == items.len() => {
                        ts.into_iter().map(Some).collect()
                    }
                    _ => items.iter().map(|_| None).collect(),
                };
                let xs: Vec<Expr> = items
                    .iter()
                    .zip(&hints)
                    .map(|(i, h)| self.expr(i, h.as_ref()))
                    .collect();
                let ty = Ty::Tuple(xs.iter().map(|x| x.ty.clone()).collect());
                mk(H::Tuple(xs), ty, span)
            }
            A::Record { ty, fields } => self.record_lit(ty, fields, span),
            A::With { base, fields } => self.with_expr(base, fields, span),
            A::Unary(op, x) => self.unary(*op, x, span),
            A::Binary(op, l, r) => self.binary(*op, l, r, span),
            A::Call { callee, args } => self.call(callee, args, span),
            A::MethodCall { recv, name, args } => self.method_call(recv, name, args, span),
            A::Field(b, name) => self.field(b, name, span),
            A::TupleField(b, i) => self.tuple_field(b, *i, span),
            A::Index(b, i) => self.index(b, i, span),
            A::Lambda { params, body } => self.lambda(params, body, exp, span),
            A::If { cond, then, els } => {
                let c = self.expr(cond, Some(&Ty::Bool));
                let t = self.expr(then, exp);
                let result = exp.cloned().unwrap_or_else(|| t.ty.clone());
                let Some(els) = els else {
                    return error_expr(span);
                };
                let f = self.expr(els, Some(&result));
                let ty = t.ty.clone();
                mk(H::If(Box::new(c), Box::new(t), Box::new(f)), ty, span)
            }
            A::Match { scrut, arms } => self.match_expr(scrut, arms, exp, span),
            A::Block { stmts, tail } => self.block(stmts, tail, exp, span),
        }
    }

    // ------------------------------------------------------------ names

    fn var(&mut self, id: &Ident, span: ByteSpan) -> Expr {
        if let Some((local, ty)) = self.lookup_local(&id.name) {
            return mk(H::Local(local), ty, span);
        }
        if let Some(&c) = self.const_names.get(&id.name) {
            if let Some(cur) = self.in_const
                && c >= cur
            {
                self.err(
                    Problem::new(
                        Code::ConstInit,
                        "a constant may only use earlier constants",
                        span,
                    )
                    .expected("a constant declared above"),
                );
            }
            let ty = self.program.consts[c].ty.clone();
            return mk(H::Const(c), ty, span);
        }
        if let Some(&f) = self.fn_names.get(&id.name) {
            self.note_call(f, span);
            let (ptys, ret, targs) = self.instantiate(f);
            return mk(H::FnRef(f, targs), Ty::func(ptys, ret), span);
        }
        self.unknown_name(id)
    }

    fn ctor_value(&mut self, id: &Ident, span: ByteSpan) -> Expr {
        let Some(&(tid, variant)) = self.ctors.get(&id.name) else {
            return self.unknown_ctor(id);
        };
        let nfields = self.program.types[tid]
            .variant_fields(variant)
            .map_or(0, <[_]>::len);
        if nfields > 0 {
            self.err(
                Problem::new(
                    Code::TypeArity,
                    format!("constructor `{}` needs {nfields} argument(s)", id.name),
                    span,
                )
                .expected(format!("`{}(..)`", id.name)),
            );
            return error_expr(span);
        }
        self.build_ctor(tid, variant, Vec::new(), span)
    }

    fn unknown_ctor(&mut self, id: &Ident) -> Expr {
        // a record type name used like a value
        if let Some(super::TypeEntry::Adt(tid)) = self.type_names.get(&id.name)
            && matches!(self.program.types[*tid].kind, hir::TypeKind::Record(_))
        {
            self.err(
                Problem::new(
                    Code::TypeArity,
                    format!("`{}` is a record; give its fields", id.name),
                    id.span,
                )
                .expected(format!("`{} {{ .. }}`", id.name)),
            );
            return error_expr(id.span);
        }
        let mut p = Problem::new(
            Code::NameUnknownCtor,
            format!("unknown constructor `{}`", id.name),
            id.span,
        );
        if let Some(s) = suggest(&id.name, self.ctors.keys().map(String::as_str)) {
            p = p
                .expected(format!("`{s}`"))
                .fix(format!("use `{s}`"), id.span, s);
        }
        self.err(p);
        error_expr(id.span)
    }

    fn build_ctor(&mut self, tid: usize, variant: usize, args: Vec<Expr>, span: ByteSpan) -> Expr {
        let nparams = self.program.types[tid].params.len();
        let targs: Vec<Ty> = (0..nparams).map(|_| self.fresh()).collect();
        let map: Vec<(String, Ty)> = self.program.types[tid]
            .params
            .iter()
            .cloned()
            .zip(targs.iter().cloned())
            .collect();
        let ftys: Vec<Ty> = self.program.types[tid]
            .variant_fields(variant)
            .map(|fs| fs.iter().map(|f| f.ty.subst_params(&map)).collect())
            .unwrap_or_default();
        for (a, t) in args.iter().zip(&ftys) {
            self.expect_ty(a.span, t, &a.ty);
        }
        mk(
            H::Ctor {
                ty: tid,
                variant,
                targs: targs.clone(),
                args,
            },
            Ty::Adt(tid, targs),
            span,
        )
    }

    fn path_value(&mut self, module: &Ident, name: &Ident, span: ByteSpan) -> Expr {
        match self
            .methods
            .get(&(module.name.clone(), name.name.clone()))
            .copied()
        {
            Some(f) if !self.program.fns[f].is_method => {
                let (ptys, ret, targs) = self.instantiate(f);
                mk(H::FnRef(f, targs), Ty::func(ptys, ret), span)
            }
            Some(_) => {
                self.err(
                    Problem::new(
                        Code::NameUseMethodSyntax,
                        format!(
                            "`{}.{}` is a method; call it as `value.{}(..)`",
                            module.name, name.name, name.name
                        ),
                        span,
                    )
                    .expected("method syntax"),
                );
                error_expr(span)
            }
            None => self.unknown_path(module, name, span),
        }
    }

    fn unknown_path(&mut self, module: &Ident, name: &Ident, span: ByteSpan) -> Expr {
        let msg = if Self::is_module(&module.name) {
            format!("`{}` has no function `{}`", module.name, name.name)
        } else {
            format!("`{}` is not a module", module.name)
        };
        let cands: Vec<&str> = self
            .methods
            .keys()
            .filter(|(m, _)| *m == module.name)
            .map(|(_, n)| n.as_str())
            .collect();
        let mut p = Problem::new(Code::NameUnknownMethod, msg, span);
        if let Some(s) = suggest(&name.name, cands.into_iter()) {
            p = p.expected(format!("`{}.{s}`", module.name));
        }
        self.err(p);
        error_expr(span)
    }

    // ------------------------------------------------------------ operators

    fn numeric(&mut self, ty: &Ty, span: ByteSpan, op: &'static str, int_only: bool) {
        match self.shallow(ty) {
            Ty::Int | Ty::Error => {}
            Ty::Float if !int_only => {}
            Ty::Var(_) => self.pending.push(Pending::Numeric {
                ty: ty.clone(),
                span,
                op,
                int_only,
            }),
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
        }
    }

    pub fn check_ordered(&mut self, ty: &Ty, span: ByteSpan) {
        match self.shallow(ty) {
            Ty::Int | Ty::Float | Ty::Str | Ty::Error => {}
            Ty::Var(_) => self.pending.push(Pending::Ordered {
                ty: ty.clone(),
                span,
            }),
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
        }
    }

    pub fn check_equatable(&mut self, ty: &Ty, span: ByteSpan) {
        let t = self.resolve(ty);
        if t.has_var() {
            self.pending.push(Pending::Equatable {
                ty: ty.clone(),
                span,
            });
        } else if t.contains_fn(&|id, args| self.adt_has_fn(id, args, 0)) {
            let found = self.show(&t);
            self.err(
                Problem::new(
                    Code::TypeNotEquatable,
                    format!("`{found}` contains functions and cannot be compared"),
                    span,
                )
                .expected("a type without functions"),
            );
        }
    }

    fn unary(&mut self, op: UnOp, x: &'a ast::Expr, span: ByteSpan) -> Expr {
        match op {
            UnOp::Neg => {
                let v = self.expr(x, None);
                self.numeric(&v.ty, span, "-", false);
                let ty = v.ty.clone();
                mk(H::Unary(op, Box::new(v)), ty, span)
            }
            UnOp::Not => {
                let v = self.expr(x, Some(&Ty::Bool));
                mk(H::Unary(op, Box::new(v)), Ty::Bool, span)
            }
        }
    }

    fn binary(&mut self, op: BinOp, l: &'a ast::Expr, r: &'a ast::Expr, span: ByteSpan) -> Expr {
        let name = op.as_str();
        match op {
            BinOp::And | BinOp::Or => {
                let a = self.expr(l, Some(&Ty::Bool));
                let b = self.expr(r, Some(&Ty::Bool));
                mk(H::Binary(op, Box::new(a), Box::new(b)), Ty::Bool, span)
            }
            BinOp::Concat => {
                let a = self.expr(l, Some(&Ty::Str));
                let b = self.expr(r, Some(&Ty::Str));
                mk(H::Binary(op, Box::new(a), Box::new(b)), Ty::Str, span)
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                let a = self.expr(l, None);
                let b = self.expr(r, Some(&a.ty));
                let int_only = op == BinOp::Rem;
                self.numeric(&a.ty, span, name, int_only);
                let ty = a.ty.clone();
                mk(H::Binary(op, Box::new(a), Box::new(b)), ty, span)
            }
            BinOp::Eq | BinOp::Ne => {
                let a = self.expr(l, None);
                let b = self.expr(r, Some(&a.ty));
                self.check_equatable(&a.ty, span);
                mk(H::Binary(op, Box::new(a), Box::new(b)), Ty::Bool, span)
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let a = self.expr(l, None);
                let b = self.expr(r, Some(&a.ty));
                self.check_ordered(&a.ty, span);
                mk(H::Binary(op, Box::new(a), Box::new(b)), Ty::Bool, span)
            }
        }
    }

    // ------------------------------------------------------------ calls

    fn call(&mut self, callee: &'a ast::Expr, args: &'a [ast::Expr], span: ByteSpan) -> Expr {
        match &callee.kind {
            A::Var(id) => {
                if self.lookup_local(&id.name).is_none() {
                    if let Some(&f) = self.fn_names.get(&id.name) {
                        return self.call_fn(f, None, args, span, callee.span);
                    }
                    if self.const_names.contains_key(&id.name) {
                        self.err(Problem::new(
                            Code::TypeNotFunction,
                            format!("`{}` is a constant, not a function", id.name),
                            callee.span,
                        ));
                        return error_expr(span);
                    }
                    return self.unknown_name(id);
                }
                let c = self.expr(callee, None);
                self.call_value(c, args, span)
            }
            A::Ctor(id) => {
                let Some(&(tid, variant)) = self.ctors.get(&id.name) else {
                    return self.unknown_ctor(id);
                };
                let ftys: Vec<Ty> = self.program.types[tid]
                    .variant_fields(variant)
                    .map(|fs| fs.iter().map(|f| f.ty.clone()).collect())
                    .unwrap_or_default();
                if ftys.len() != args.len() {
                    self.err(
                        Problem::new(
                            Code::TypeArity,
                            format!(
                                "constructor `{}` takes {} argument(s), found {}",
                                id.name,
                                ftys.len(),
                                args.len()
                            ),
                            span,
                        )
                        .expected(format!("{} argument(s)", ftys.len())),
                    );
                    return error_expr(span);
                }
                // check arguments against the instantiated payload types
                let nparams = self.program.types[tid].params.len();
                let targs: Vec<Ty> = (0..nparams).map(|_| self.fresh()).collect();
                let map: Vec<(String, Ty)> = self.program.types[tid]
                    .params
                    .iter()
                    .cloned()
                    .zip(targs.iter().cloned())
                    .collect();
                let hargs: Vec<Expr> = args
                    .iter()
                    .zip(&ftys)
                    .map(|(a, t)| self.expr(a, Some(&t.subst_params(&map))))
                    .collect();
                mk(
                    H::Ctor {
                        ty: tid,
                        variant,
                        targs: targs.clone(),
                        args: hargs,
                    },
                    Ty::Adt(tid, targs),
                    span,
                )
            }
            A::Path { module, name } => {
                if !Self::is_module(&module.name) && self.ctors.contains_key(&module.name) {
                    // `None.unwrapOr(1)`: a method call on a constructor value
                    let r = self.ctor_value(module, module.span);
                    return self.method_call_on(r, name, args, span);
                }
                let Some(&f) = self.methods.get(&(module.name.clone(), name.name.clone())) else {
                    return self.unknown_path(module, name, callee.span);
                };
                if self.program.fns[f].is_method {
                    return self.method_via_path(f, module, name, args, span);
                }
                self.call_fn(f, None, args, span, callee.span)
            }
            _ => {
                let c = self.expr(callee, None);
                self.call_value(c, args, span)
            }
        }
    }

    /// `List.map(xs, f)` written instead of `xs.map(f)`.
    fn method_via_path(
        &mut self,
        f: usize,
        module: &Ident,
        name: &Ident,
        args: &'a [ast::Expr],
        span: ByteSpan,
    ) -> Expr {
        let Some((first, rest)) = args.split_first() else {
            self.err(
                Problem::new(
                    Code::TypeArity,
                    format!("`{}.{}` needs a receiver", module.name, name.name),
                    span,
                )
                .expected(self.sig_string(f)),
            );
            return error_expr(span);
        };
        let simple = matches!(
            first.kind,
            A::Var(_)
                | A::Call { .. }
                | A::MethodCall { .. }
                | A::Field(..)
                | A::TupleField(..)
                | A::Index(..)
                | A::List(_)
                | A::Tuple(_)
                | A::Paren(_)
                | A::Int(_)
                | A::Str(_)
                | A::Bool(_)
                | A::Ctor(_)
                | A::Path { .. }
                | A::Record { .. }
        );
        let recv_text = self.text(first.span);
        let recv = if simple {
            recv_text.to_string()
        } else {
            format!("({recv_text})")
        };
        let rest_text: Vec<&str> = rest.iter().map(|a| self.text(a.span)).collect();
        let fixed = format!("{recv}.{}({})", name.name, rest_text.join(", "));
        self.err(
            Problem::new(
                Code::NameUseMethodSyntax,
                format!(
                    "`{}` is a method: write `value.{}(..)`",
                    name.name, name.name
                ),
                span,
            )
            .expected(format!("`{fixed}`"))
            .fix("use method syntax", span, fixed),
        );
        let r = self.expr(first, None);
        self.call_fn(f, Some(r), rest, span, span)
    }

    fn method_call(
        &mut self,
        recv: &'a ast::Expr,
        name: &Ident,
        args: &'a [ast::Expr],
        span: ByteSpan,
    ) -> Expr {
        let r = self.expr(recv, None);
        self.method_call_on(r, name, args, span)
    }

    fn method_call_on(
        &mut self,
        r: Expr,
        name: &Ident,
        args: &'a [ast::Expr],
        span: ByteSpan,
    ) -> Expr {
        let recv_span = r.span;
        let rt = self.shallow(&r.ty);
        let Some(module) = self.module_of(&rt) else {
            match rt {
                Ty::Error => {
                    for a in args {
                        let _ = self.expr(a, None);
                    }
                }
                Ty::Var(_) => self.err(
                    Problem::new(
                        Code::TypeAmbiguous,
                        format!(
                            "the type of the receiver of `.{}` is not known here",
                            name.name
                        ),
                        recv_span,
                    )
                    .expected("a type annotation, e.g. `let xs: List[Int] = ..`"),
                ),
                other => {
                    let t = self.show(&other);
                    // `b.f(x)` where `f` is a function-typed field: the call needs the field in parentheses
                    let fn_field = match &other {
                        Ty::Adt(id, _) => self
                            .record_fields(*id)
                            .iter()
                            .any(|(n, ty)| n == &name.name && matches!(ty, Ty::Fn(..))),
                        _ => false,
                    };
                    if fn_field {
                        let whole = ByteSpan::new(recv_span.start, name.span.end);
                        let text = self
                            .src
                            .get(whole.start..whole.end)
                            .unwrap_or_default()
                            .to_string();
                        self.err(
                            Problem::new(
                                Code::NameUnknownMethod,
                                format!(
                                    "`{}` is a field holding a function, not a method; call it as `({text})(..)`",
                                    name.name
                                ),
                                name.span,
                            )
                            .expected(format!("`({text})(..)`"))
                            .fix("parenthesize the field", whole, format!("({text})")),
                        );
                    } else {
                        self.err(Problem::new(
                            Code::NameUnknownMethod,
                            format!(
                                "values of type `{t}` have no methods; `.{}` is not available",
                                name.name
                            ),
                            name.span,
                        ));
                    }
                }
            }
            return error_expr(span);
        };
        match self
            .methods
            .get(&(module.to_string(), name.name.clone()))
            .copied()
        {
            Some(f) if self.program.fns[f].is_method => {
                self.call_fn(f, Some(r), args, span, name.span)
            }
            _ => {
                let cands: Vec<&str> = self
                    .methods
                    .iter()
                    .filter(|((m, _), id)| m == module && self.program.fns[**id].is_method)
                    .map(|((_, n), _)| n.as_str())
                    .collect();
                let mut p = Problem::new(
                    Code::NameUnknownMethod,
                    format!("`{module}` has no method `{}`", name.name),
                    name.span,
                );
                if let Some(s) = suggest(&name.name, cands.iter().copied()) {
                    p = p
                        .expected(format!("`{s}`"))
                        .fix(format!("use `{s}`"), name.span, s);
                } else {
                    let mut all = cands;
                    all.sort_unstable();
                    p = p.expected(format!("one of: {}", all.join(", ")));
                }
                self.err(p);
                error_expr(span)
            }
        }
    }

    /// Checks a call to function `f`; `recv` is an already-checked receiver.
    fn call_fn(
        &mut self,
        f: usize,
        recv: Option<Expr>,
        args: &'a [ast::Expr],
        span: ByteSpan,
        name_span: ByteSpan,
    ) -> Expr {
        self.note_call(f, name_span);
        let (ptys, ret, targs) = self.instantiate(f);
        let total = args.len() + usize::from(recv.is_some());
        if total != ptys.len() {
            let sig = self.sig_string(f);
            let skip = usize::from(recv.is_some() && self.program.fns[f].is_method);
            self.err(
                Problem::new(
                    Code::TypeArity,
                    format!(
                        "`{}` takes {} argument(s), found {}",
                        self.program.fns[f].full_name(),
                        ptys.len().saturating_sub(skip),
                        total.saturating_sub(skip)
                    ),
                    span,
                )
                .expected(format!("`{sig}`")),
            );
            for a in args {
                let _ = self.expr(a, None);
            }
            return error_expr(span);
        }
        let mut hargs = Vec::with_capacity(total);
        let mut ptys_iter = ptys.iter();
        if let Some(r) = recv {
            if let Some(pt) = ptys_iter.next() {
                self.expect_ty(r.span, pt, &r.ty);
            }
            hargs.push(r);
        }
        for (a, pt) in args.iter().zip(ptys_iter) {
            hargs.push(self.expr(a, Some(pt)));
        }
        mk(
            H::Call {
                f,
                targs,
                args: hargs,
            },
            ret,
            span,
        )
    }

    fn call_value(&mut self, callee: Expr, args: &'a [ast::Expr], span: ByteSpan) -> Expr {
        let ct = self.shallow(&callee.ty);
        let (ptys, ret) = match ct {
            Ty::Fn(ps, r) => (ps, *r),
            Ty::Var(_) => {
                let ps: Vec<Ty> = args.iter().map(|_| self.fresh()).collect();
                let r = self.fresh();
                let f = Ty::func(ps.clone(), r.clone());
                self.expect_ty(callee.span, &callee.ty, &f);
                (ps, r)
            }
            Ty::Error => {
                for a in args {
                    let _ = self.expr(a, None);
                }
                return error_expr(span);
            }
            other => {
                let t = self.show(&other);
                self.err(Problem::new(
                    Code::TypeNotFunction,
                    format!("a value of type `{t}` cannot be called"),
                    callee.span,
                ));
                for a in args {
                    let _ = self.expr(a, None);
                }
                return error_expr(span);
            }
        };
        if ptys.len() != args.len() {
            let t = self.show(&Ty::func(ptys.clone(), ret));
            self.err(
                Problem::new(
                    Code::TypeArity,
                    format!(
                        "this function takes {} argument(s), found {}",
                        ptys.len(),
                        args.len()
                    ),
                    span,
                )
                .expected(format!("`{t}`")),
            );
            for a in args {
                let _ = self.expr(a, None);
            }
            return error_expr(span);
        }
        let hargs: Vec<Expr> = args
            .iter()
            .zip(&ptys)
            .map(|(a, t)| self.expr(a, Some(t)))
            .collect();
        mk(
            H::CallValue {
                callee: Box::new(callee),
                args: hargs,
            },
            ret,
            span,
        )
    }

    // ------------------------------------------------------------ records and fields

    fn record_type(&mut self, ty: &Ty, span: ByteSpan, what: &str) -> Option<(usize, Vec<Ty>)> {
        match self.shallow(ty) {
            Ty::Adt(id, args)
                if matches!(self.program.types[id].kind, hir::TypeKind::Record(_)) =>
            {
                Some((id, args))
            }
            Ty::Error => None,
            Ty::Var(_) => {
                self.err(
                    Problem::new(
                        Code::TypeAmbiguous,
                        format!("the type of this value is not known, so {what} is ambiguous"),
                        span,
                    )
                    .expected("a type annotation"),
                );
                None
            }
            other => {
                let t = self.show(&other);
                self.err(Problem::new(
                    Code::TypeUnknownField,
                    format!("`{t}` is not a record, so {what} is not available"),
                    span,
                ));
                None
            }
        }
    }

    fn record_fields(&self, id: usize) -> Vec<(String, Ty)> {
        match &self.program.types[id].kind {
            hir::TypeKind::Record(fs) => {
                fs.iter().map(|f| (f.name.clone(), f.ty.clone())).collect()
            }
            hir::TypeKind::Adt(_) => Vec::new(),
        }
    }

    fn record_lit(&mut self, tyname: &Ident, fields: &'a [ast::FieldInit], span: ByteSpan) -> Expr {
        let Some(super::TypeEntry::Adt(tid)) = self.type_names.get(&tyname.name) else {
            let mut p = Problem::new(
                Code::NameUnknownType,
                format!("unknown record type `{}`", tyname.name),
                tyname.span,
            );
            if let Some(s) = suggest(&tyname.name, self.type_names.keys().map(String::as_str)) {
                p = p
                    .expected(format!("`{s}`"))
                    .fix(format!("use `{s}`"), tyname.span, s);
            }
            self.err(p);
            for f in fields {
                let _ = self.expr(&f.value, None);
            }
            return error_expr(span);
        };
        let tid = *tid;
        if !matches!(self.program.types[tid].kind, hir::TypeKind::Record(_)) {
            self.err(Problem::new(
                Code::TypeUnknownField,
                format!("`{}` is not a record type", tyname.name),
                tyname.span,
            ));
            return error_expr(span);
        }
        let nparams = self.program.types[tid].params.len();
        let targs: Vec<Ty> = (0..nparams).map(|_| self.fresh()).collect();
        let map: Vec<(String, Ty)> = self.program.types[tid]
            .params
            .iter()
            .cloned()
            .zip(targs.iter().cloned())
            .collect();
        let decl = self.record_fields(tid);
        let mut slots: Vec<Option<Expr>> = decl.iter().map(|_| None).collect();
        for fi in fields {
            match decl.iter().position(|(n, _)| *n == fi.name.name) {
                Some(i) if slots[i].is_none() => {
                    let ft = decl[i].1.subst_params(&map);
                    slots[i] = Some(self.expr(&fi.value, Some(&ft)));
                }
                Some(_) => {
                    self.err(Problem::new(
                        Code::TypeExtraField,
                        format!("field `{}` is given twice", fi.name.name),
                        fi.name.span,
                    ));
                    let _ = self.expr(&fi.value, None);
                }
                None => {
                    let names: Vec<&str> = decl.iter().map(|(n, _)| n.as_str()).collect();
                    self.err(
                        Problem::new(
                            Code::TypeExtraField,
                            format!("`{}` has no field `{}`", tyname.name, fi.name.name),
                            fi.name.span,
                        )
                        .expected(format!("one of: {}", names.join(", "))),
                    );
                    let _ = self.expr(&fi.value, None);
                }
            }
        }
        let missing: Vec<&str> = decl
            .iter()
            .zip(&slots)
            .filter(|(_, s)| s.is_none())
            .map(|((n, _), _)| n.as_str())
            .collect();
        if !missing.is_empty() {
            self.err(
                Problem::new(
                    Code::TypeMissingField,
                    format!(
                        "`{}` literal is missing: {}",
                        tyname.name,
                        missing.join(", ")
                    ),
                    span,
                )
                .expected(format!("fields: {}", missing.join(", "))),
            );
            return error_expr(span);
        }
        let args: Vec<Expr> = slots.into_iter().flatten().collect();
        mk(
            H::Ctor {
                ty: tid,
                variant: 0,
                targs: targs.clone(),
                args,
            },
            Ty::Adt(tid, targs),
            span,
        )
    }

    fn with_expr(
        &mut self,
        base: &'a ast::Expr,
        fields: &'a [ast::FieldInit],
        span: ByteSpan,
    ) -> Expr {
        let b = self.expr(base, None);
        let Some((tid, targs)) = self.record_type(&b.ty, base.span, "`with`") else {
            for f in fields {
                let _ = self.expr(&f.value, None);
            }
            return error_expr(span);
        };
        let map: Vec<(String, Ty)> = self.program.types[tid]
            .params
            .iter()
            .cloned()
            .zip(targs.iter().cloned())
            .collect();
        let decl = self.record_fields(tid);
        let mut updates: Vec<(usize, Expr)> = Vec::new();
        for fi in fields {
            let Some(i) = decl.iter().position(|(n, _)| *n == fi.name.name) else {
                let names: Vec<&str> = decl.iter().map(|(n, _)| n.as_str()).collect();
                self.err(
                    Problem::new(
                        Code::TypeUnknownField,
                        format!(
                            "`{}` has no field `{}`",
                            self.program.type_name(tid),
                            fi.name.name
                        ),
                        fi.name.span,
                    )
                    .expected(format!("one of: {}", names.join(", "))),
                );
                let _ = self.expr(&fi.value, None);
                continue;
            };
            if updates.iter().any(|(j, _)| *j == i) {
                self.err(Problem::new(
                    Code::TypeExtraField,
                    format!("field `{}` is given twice", fi.name.name),
                    fi.name.span,
                ));
                continue;
            }
            let ft = decl[i].1.subst_params(&map);
            updates.push((i, self.expr(&fi.value, Some(&ft))));
        }
        let ty = b.ty.clone();
        mk(H::With(Box::new(b), updates), ty, span)
    }

    fn field(&mut self, base: &'a ast::Expr, name: &Ident, span: ByteSpan) -> Expr {
        let b = self.expr(base, None);
        let Some((tid, targs)) = self.record_type(&b.ty, span, &format!("`.{}`", name.name)) else {
            return error_expr(span);
        };
        let map: Vec<(String, Ty)> = self.program.types[tid]
            .params
            .iter()
            .cloned()
            .zip(targs)
            .collect();
        let decl = self.record_fields(tid);
        match decl.iter().position(|(n, _)| *n == name.name) {
            Some(i) => {
                let ty = decl[i].1.subst_params(&map);
                mk(H::Field(Box::new(b), i), ty, span)
            }
            None => {
                let names: Vec<&str> = decl.iter().map(|(n, _)| n.as_str()).collect();
                let mut p = Problem::new(
                    Code::TypeUnknownField,
                    format!(
                        "`{}` has no field `{}`",
                        self.program.type_name(tid),
                        name.name
                    ),
                    name.span,
                );
                if let Some(s) = suggest(&name.name, names.iter().copied()) {
                    p = p
                        .expected(format!("`{s}`"))
                        .fix(format!("use `{s}`"), name.span, s);
                } else {
                    p = p.expected(format!("one of: {}", names.join(", ")));
                }
                self.err(p);
                error_expr(span)
            }
        }
    }

    fn tuple_field(&mut self, base: &'a ast::Expr, idx: u32, span: ByteSpan) -> Expr {
        let b = self.expr(base, None);
        match self.shallow(&b.ty) {
            Ty::Tuple(ts) => match ts.get(idx as usize) {
                Some(t) => {
                    let t = t.clone();
                    mk(H::TupleField(Box::new(b), idx as usize), t, span)
                }
                None => {
                    self.err(
                        Problem::new(
                            Code::TypeTupleIndex,
                            format!("a tuple of {} elements has no field {idx}", ts.len()),
                            span,
                        )
                        .expected(format!("an index below {}", ts.len())),
                    );
                    error_expr(span)
                }
            },
            Ty::Error => error_expr(span),
            Ty::Var(_) => {
                self.err(
                    Problem::new(
                        Code::TypeAmbiguous,
                        "the type of this tuple is not known here",
                        span,
                    )
                    .expected("a type annotation"),
                );
                error_expr(span)
            }
            other => {
                let t = self.show(&other);
                self.err(Problem::new(
                    Code::TypeTupleIndex,
                    format!("`{t}` is not a tuple"),
                    span,
                ));
                error_expr(span)
            }
        }
    }

    fn index(&mut self, base: &'a ast::Expr, idx: &'a ast::Expr, span: ByteSpan) -> Expr {
        let b = self.expr(base, None);
        let i = self.expr(idx, Some(&Ty::Int));
        let elem = match self.shallow(&b.ty) {
            Ty::List(t) => *t,
            Ty::Error => Ty::Error,
            Ty::Var(_) => {
                self.err(
                    Problem::new(
                        Code::TypeAmbiguous,
                        "the type of this value is not known, so `[..]` is ambiguous",
                        span,
                    )
                    .expected("a type annotation"),
                );
                Ty::Error
            }
            other => {
                let t = self.show(&other);
                self.err(
                    Problem::new(
                        Code::TypeBadOperand,
                        format!("`[..]` needs a List, found `{t}`"),
                        base.span,
                    )
                    .expected("List[T]"),
                );
                Ty::Error
            }
        };
        mk(H::Index(Box::new(b), Box::new(i)), elem, span)
    }

    // ------------------------------------------------------------ lambda / match / block

    fn lambda(
        &mut self,
        params: &'a [ast::LambdaParam],
        body: &'a ast::Expr,
        exp: Option<&Ty>,
        span: ByteSpan,
    ) -> Expr {
        let (hints, ret_hint): (Vec<Ty>, Option<Ty>) = match exp.map(|t| self.shallow(t)) {
            Some(Ty::Fn(ps, r)) if ps.len() == params.len() => (ps, Some(*r)),
            Some(Ty::Fn(ps, _)) => {
                self.err(
                    Problem::new(
                        Code::TypeArity,
                        format!(
                            "this function must take {} parameter(s), found {}",
                            ps.len(),
                            params.len()
                        ),
                        span,
                    )
                    .expected(format!("{} parameter(s)", ps.len())),
                );
                (params.iter().map(|_| self.fresh()).collect(), None)
            }
            _ => (params.iter().map(|_| self.fresh()).collect(), None),
        };
        self.push_scope();
        let mut locals: Vec<LocalId> = Vec::new();
        let mut ptys: Vec<Ty> = Vec::new();
        for (p, hint) in params.iter().zip(hints) {
            let ty = if let Some(te) = &p.ty {
                let t = self.resolve_type(te, false).ty;
                self.expect_ty(te.span, &hint, &t);
                t
            } else {
                hint
            };
            if p.name.name == "_" {
                locals.push(self.new_local());
            } else {
                locals.push(self.declare_local(&p.name, ty.clone()));
            }
            ptys.push(ty);
        }
        let b = self.expr(body, ret_hint.as_ref());
        self.pop_scope();
        let ty = Ty::func(ptys, b.ty.clone());
        mk(
            H::Lambda {
                params: locals,
                body: Box::new(b),
            },
            ty,
            span,
        )
    }

    fn match_expr(
        &mut self,
        scrut: &'a ast::Expr,
        arms: &'a [ast::Arm],
        exp: Option<&Ty>,
        span: ByteSpan,
    ) -> Expr {
        let s = self.expr(scrut, None);
        let result = match exp {
            Some(t) => t.clone(),
            None => self.fresh(),
        };
        let mut harms = Vec::new();
        for arm in arms {
            self.push_scope();
            let pat = self.pattern(&arm.pat, &s.ty);
            let guard = arm.guard.as_ref().map(|g| self.expr(g, Some(&Ty::Bool)));
            let body = self.expr(&arm.body, Some(&result));
            self.pop_scope();
            harms.push(hir::Arm { pat, guard, body });
        }
        mk(H::Match(Box::new(s), harms), result, span)
    }

    fn block(
        &mut self,
        stmts: &'a [ast::Stmt],
        tail: &'a ast::Expr,
        exp: Option<&Ty>,
        span: ByteSpan,
    ) -> Expr {
        self.push_scope();
        let mut hs = Vec::new();
        for s in stmts {
            match s {
                ast::Stmt::Let {
                    pat,
                    ty,
                    init,
                    span,
                } => {
                    let (annot, refine) = match ty {
                        Some(te) => {
                            let r = self.resolve_type(te, true);
                            let rs = self.lower_refines(&r.refine);
                            (Some(r.ty), rs)
                        }
                        None => (None, Vec::new()),
                    };
                    let i = self.expr(init, annot.as_ref());
                    let p = self.pattern(pat, &i.ty);
                    hs.push(hir::Stmt::Let {
                        pat: p,
                        init: i,
                        refine,
                        span: *span,
                    });
                }
                ast::Stmt::Assert { cond, .. } => {
                    let c = self.expr(cond, Some(&Ty::Bool));
                    hs.push(hir::Stmt::Assert(c));
                }
            }
        }
        let t = self.expr(tail, exp);
        self.pop_scope();
        let ty = t.ty.clone();
        mk(H::Block(hs, Box::new(t)), ty, span)
    }
}
