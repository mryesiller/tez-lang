//! The evaluator.

use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};
use tez_diag::Code;
use tez_syntax::ByteSpan;
use tez_types::hir::{
    BinOp, Expr, ExprKind, FnId, LitVal, Pat, PatKind, Program, Refinement, Stmt, UnOp,
};

use crate::value::{Closure, ListVal, Value, values_cmp, values_eq};

/// Resource limits and checking options.
#[derive(Debug, Clone)]
pub struct Options {
    /// Maximum call depth before `E_RESOURCE`.
    pub max_depth: usize,
    /// Maximum number of evaluation steps before `E_RESOURCE`.
    pub fuel: u64,
    /// Evaluate `requires`, `ensures` and refinements at run time. Unverified programs only:
    /// verified programs cannot violate them.
    pub check_contracts: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            max_depth: 20_000,
            fuel: 2_000_000_000,
            check_contracts: false,
        }
    }
}

/// A run-time failure.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    /// Stable code (verification codes mean "this would have been rejected if verified").
    pub code: Code,
    /// Short message.
    pub message: String,
    /// Where it happened, when known.
    pub span: ByteSpan,
}

pub(crate) type R<T> = Result<T, RuntimeError>;

pub(crate) fn rt(code: Code, message: impl Into<String>, span: ByteSpan) -> RuntimeError {
    RuntimeError {
        code,
        message: message.into(),
        span,
    }
}

pub(crate) type Frame<'p> = Vec<Value<'p>>;

/// An interpreter bound to one checked program.
pub struct Interp<'p> {
    pub(crate) prog: &'p Program,
    pub(crate) opts: Options,
    consts: Vec<Option<Value<'p>>>,
    depth: usize,
    steps: u64,
    pub(crate) option_ty: usize,
}

impl<'p> Interp<'p> {
    /// Creates an interpreter.
    #[must_use]
    pub fn new(prog: &'p Program, opts: Options) -> Self {
        let option_ty = prog
            .types
            .iter()
            .position(|t| t.is_prelude && t.name == "Option")
            .unwrap_or(0);
        Self {
            prog,
            opts,
            consts: vec![None; prog.consts.len()],
            depth: 0,
            steps: 0,
            option_ty,
        }
    }

    pub(crate) fn none(&self) -> Value<'p> {
        Value::Adt {
            ty: self.option_ty,
            variant: 0,
            fields: Rc::new(Vec::new()),
        }
    }

    pub(crate) fn some(&self, v: Value<'p>) -> Value<'p> {
        Value::Adt {
            ty: self.option_ty,
            variant: 1,
            fields: Rc::new(vec![v]),
        }
    }

    fn tick(&mut self, span: ByteSpan) -> R<()> {
        self.steps += 1;
        if self.steps > self.opts.fuel {
            return Err(rt(Code::Resource, "evaluation step limit exceeded", span));
        }
        Ok(())
    }

    // ------------------------------------------------------------ entry points

    /// Calls a function by id.
    ///
    /// # Errors
    /// Returns the run-time failure, if any.
    pub fn call(&mut self, f: FnId, args: Vec<Value<'p>>) -> R<Value<'p>> {
        let def = &self.prog.fns[f];
        if self.depth >= self.opts.max_depth {
            return Err(rt(Code::Resource, "call depth limit exceeded", def.span));
        }
        if def.is_extern {
            let name = def.full_name();
            return crate::native::call(self, &name, args, def.span);
        }
        let Some(body) = &def.body else {
            return Err(rt(Code::Internal, "function without a body", def.span));
        };
        self.depth += 1;
        let r = self.call_user(f, body, args);
        self.depth -= 1;
        r
    }

    fn call_user(&mut self, f: FnId, body: &'p Expr, args: Vec<Value<'p>>) -> R<Value<'p>> {
        let def = &self.prog.fns[f];
        let mut fr: Frame<'p> = vec![Value::Unit; def.nlocals as usize];
        for (p, a) in def.params.iter().zip(args) {
            fr[p.local as usize] = a;
        }
        if self.opts.check_contracts {
            for p in &def.params {
                let v = fr[p.local as usize].clone();
                self.check_refines(
                    &p.refine,
                    &v,
                    &mut fr,
                    Code::Refinement,
                    "argument violates its refined type",
                )?;
            }
            for r in &def.requires {
                if !self.truthy(r, &mut fr)? {
                    return Err(rt(Code::Precondition, "`requires` does not hold", r.span));
                }
            }
        }
        let result = self.eval(body, &mut fr)?;
        if self.opts.check_contracts {
            self.check_refines(
                &def.ret_refine,
                &result,
                &mut fr,
                Code::Refinement,
                "result violates its refined type",
            )?;
            fr[def.result_local as usize] = result.clone();
            for e in &def.ensures {
                if !self.truthy(e, &mut fr)? {
                    return Err(rt(Code::Postcondition, "`ensures` does not hold", e.span));
                }
            }
        }
        Ok(result)
    }

    fn truthy(&mut self, e: &'p Expr, fr: &mut Frame<'p>) -> R<bool> {
        match self.eval(e, fr)? {
            Value::Bool(b) => Ok(b),
            _ => Err(rt(Code::Internal, "predicate is not a Bool", e.span)),
        }
    }

    /// Evaluates refinements whose predicates live in the current frame.
    fn check_refines(
        &mut self,
        rs: &'p [Refinement],
        v: &Value<'p>,
        fr: &mut Frame<'p>,
        code: Code,
        msg: &str,
    ) -> R<()> {
        for r in rs {
            fr[r.it as usize] = v.clone();
            if !self.truthy(&r.pred, fr)? {
                return Err(rt(code, msg, r.pred.span));
            }
        }
        Ok(())
    }

    /// Evaluates type-level refinements, which own their frame.
    fn check_own_frame_refines(&mut self, rs: &'p [Refinement], v: &Value<'p>, msg: &str) -> R<()> {
        for r in rs {
            let mut fr: Frame<'p> = vec![Value::Unit; r.nlocals.max(r.it + 1) as usize];
            fr[r.it as usize] = v.clone();
            if !self.truthy(&r.pred, &mut fr)? {
                return Err(rt(Code::Refinement, msg, r.pred.span));
            }
        }
        Ok(())
    }

    fn const_value(&mut self, id: usize) -> R<Value<'p>> {
        if let Some(v) = &self.consts[id] {
            return Ok(v.clone());
        }
        let c = &self.prog.consts[id];
        let mut fr: Frame<'p> = vec![Value::Unit; c.nlocals as usize];
        let v = self.eval(&c.value, &mut fr)?;
        if self.opts.check_contracts {
            self.check_refines(
                &c.refine,
                &v,
                &mut fr,
                Code::Refinement,
                "constant violates its refined type",
            )?;
        }
        self.consts[id] = Some(v.clone());
        Ok(v)
    }

    pub(crate) fn call_value(
        &mut self,
        f: &Value<'p>,
        args: Vec<Value<'p>>,
        span: ByteSpan,
    ) -> R<Value<'p>> {
        match f {
            Value::FnRef(id) => self.call(*id, args),
            Value::Closure(c) => {
                if self.depth >= self.opts.max_depth {
                    return Err(rt(Code::Resource, "call depth limit exceeded", span));
                }
                let mut fr = c.captured.clone();
                for (slot, a) in c.params.iter().zip(args) {
                    fr[*slot as usize] = a;
                }
                self.depth += 1;
                let r = self.eval(c.body, &mut fr);
                self.depth -= 1;
                r
            }
            _ => Err(rt(Code::Internal, "calling a non-function", span)),
        }
    }

    // ------------------------------------------------------------ evaluation

    pub(crate) fn eval(&mut self, e: &'p Expr, fr: &mut Frame<'p>) -> R<Value<'p>> {
        self.tick(e.span)?;
        match &e.kind {
            ExprKind::Int(b) => Ok(Value::from_big(b.clone())),
            ExprKind::Float(f) => Ok(Value::Float(*f)),
            ExprKind::Str(s) => Ok(Value::Str(Rc::from(s.as_str()))),
            ExprKind::Bool(b) => Ok(Value::Bool(*b)),
            ExprKind::Unit => Ok(Value::Unit),
            ExprKind::Local(i) => Ok(fr[*i as usize].clone()),
            ExprKind::Const(c) => self.const_value(*c),
            ExprKind::FnRef(f, _) => Ok(Value::FnRef(*f)),
            ExprKind::Call { f, args, .. } => {
                let mut vs = Vec::with_capacity(args.len());
                for a in args {
                    vs.push(self.eval(a, fr)?);
                }
                self.call(*f, vs)
            }
            ExprKind::CallValue { callee, args } => {
                let c = self.eval(callee, fr)?;
                let mut vs = Vec::with_capacity(args.len());
                for a in args {
                    vs.push(self.eval(a, fr)?);
                }
                self.call_value(&c, vs, e.span)
            }
            ExprKind::Ctor {
                ty, variant, args, ..
            } => {
                let mut vs = Vec::with_capacity(args.len());
                for a in args {
                    vs.push(self.eval(a, fr)?);
                }
                if self.opts.check_contracts {
                    self.check_fields(*ty, *variant, &vs)?;
                }
                Ok(Value::Adt {
                    ty: *ty,
                    variant: *variant,
                    fields: Rc::new(vs),
                })
            }
            ExprKind::Field(b, i) => match self.eval(b, fr)? {
                Value::Adt { fields, .. } => Ok(fields[*i].clone()),
                _ => Err(rt(Code::Internal, "field of a non-record", e.span)),
            },
            ExprKind::TupleField(b, i) => match self.eval(b, fr)? {
                Value::Tuple(vs) => Ok(vs[*i].clone()),
                _ => Err(rt(Code::Internal, "field of a non-tuple", e.span)),
            },
            ExprKind::With(b, ups) => {
                let Value::Adt {
                    ty,
                    variant,
                    fields,
                } = self.eval(b, fr)?
                else {
                    return Err(rt(Code::Internal, "`with` on a non-record", e.span));
                };
                let mut vs = (*fields).clone();
                for (i, ue) in ups {
                    vs[*i] = self.eval(ue, fr)?;
                }
                if self.opts.check_contracts {
                    self.check_fields(ty, variant, &vs)?;
                }
                Ok(Value::Adt {
                    ty,
                    variant,
                    fields: Rc::new(vs),
                })
            }
            ExprKind::Tuple(es) => {
                let mut vs = Vec::with_capacity(es.len());
                for x in es {
                    vs.push(self.eval(x, fr)?);
                }
                Ok(Value::Tuple(Rc::new(vs)))
            }
            ExprKind::List(es) => {
                let mut vs = Vec::with_capacity(es.len());
                for x in es {
                    vs.push(self.eval(x, fr)?);
                }
                Ok(Value::List(ListVal::new(vs)))
            }
            ExprKind::Index(b, i) => {
                let Value::List(l) = self.eval(b, fr)? else {
                    return Err(rt(Code::Internal, "indexing a non-list", e.span));
                };
                let iv = self.eval(i, fr)?;
                match iv.to_big().and_then(|b| b.to_usize()) {
                    Some(k) if k < l.len() => Ok(l.as_slice()[k].clone()),
                    _ => Err(rt(Code::IndexBounds, "index out of range", e.span)),
                }
            }
            ExprKind::Unary(op, x) => {
                let v = self.eval(x, fr)?;
                match (op, v) {
                    (UnOp::Not, Value::Bool(b)) => Ok(Value::Bool(!b)),
                    (UnOp::Neg, Value::Int(i)) => Ok(i
                        .checked_neg()
                        .map_or_else(|| Value::from_big(-BigInt::from(i)), Value::Int)),
                    (UnOp::Neg, Value::Big(b)) => Ok(Value::from_big(-(*b).clone())),
                    (UnOp::Neg, Value::Float(f)) => Ok(Value::Float(-f)),
                    _ => Err(rt(Code::Internal, "bad unary operand", e.span)),
                }
            }
            ExprKind::Binary(op, l, r) => self.binary(*op, l, r, e.span, fr),
            ExprKind::If(c, t, f) => {
                if self.truthy(c, fr)? {
                    self.eval(t, fr)
                } else {
                    self.eval(f, fr)
                }
            }
            ExprKind::Match(s, arms) => {
                let v = self.eval(s, fr)?;
                for arm in arms {
                    if self.match_pat(&arm.pat, &v, fr) {
                        if let Some(g) = &arm.guard
                            && !self.truthy(g, fr)?
                        {
                            continue;
                        }
                        return self.eval(&arm.body, fr);
                    }
                }
                Err(rt(Code::Internal, "no match arm applied", e.span))
            }
            ExprKind::Block(stmts, tail) => {
                for s in stmts {
                    match s {
                        Stmt::Let {
                            pat,
                            init,
                            refine,
                            span,
                        } => {
                            let v = self.eval(init, fr)?;
                            if self.opts.check_contracts {
                                self.check_refines(
                                    refine,
                                    &v,
                                    fr,
                                    Code::Refinement,
                                    "value violates its refined type",
                                )?;
                            }
                            if !self.match_pat(pat, &v, fr) {
                                return Err(rt(
                                    Code::Internal,
                                    "irrefutable pattern failed",
                                    *span,
                                ));
                            }
                        }
                        Stmt::Assert(c) => {
                            if !self.truthy(c, fr)? {
                                return Err(rt(Code::Assert, "assertion failed", c.span));
                            }
                        }
                    }
                }
                self.eval(tail, fr)
            }
            ExprKind::Lambda { params, body } => Ok(Value::Closure(Rc::new(Closure {
                params,
                body,
                captured: fr.clone(),
            }))),
        }
    }

    fn check_fields(&mut self, ty: usize, variant: usize, vs: &[Value<'p>]) -> R<()> {
        let prog = self.prog;
        if let Some(fields) = prog.types[ty].variant_fields(variant) {
            for (f, v) in fields.iter().zip(vs) {
                if !f.refine.is_empty() {
                    self.check_own_frame_refines(&f.refine, v, "field violates its refined type")?;
                }
            }
        }
        Ok(())
    }

    fn binary(
        &mut self,
        op: BinOp,
        l: &'p Expr,
        r: &'p Expr,
        span: ByteSpan,
        fr: &mut Frame<'p>,
    ) -> R<Value<'p>> {
        match op {
            BinOp::And => {
                return Ok(Value::Bool(self.truthy(l, fr)? && self.truthy(r, fr)?));
            }
            BinOp::Or => {
                return Ok(Value::Bool(self.truthy(l, fr)? || self.truthy(r, fr)?));
            }
            _ => {}
        }
        let a = self.eval(l, fr)?;
        let b = self.eval(r, fr)?;
        match op {
            BinOp::Eq => Ok(Value::Bool(values_eq(&a, &b))),
            BinOp::Ne => Ok(Value::Bool(!values_eq(&a, &b))),
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let ord = values_cmp(&a, &b)
                    .ok_or_else(|| rt(Code::Internal, "values are not ordered", span))?;
                Ok(Value::Bool(match op {
                    BinOp::Lt => ord.is_lt(),
                    BinOp::Le => ord.is_le(),
                    BinOp::Gt => ord.is_gt(),
                    _ => ord.is_ge(),
                }))
            }
            BinOp::Concat => match (a, b) {
                (Value::Str(x), Value::Str(y)) => {
                    let mut s = String::with_capacity(x.len() + y.len());
                    s.push_str(&x);
                    s.push_str(&y);
                    Ok(Value::Str(Rc::from(s)))
                }
                _ => Err(rt(Code::Internal, "`++` on non-strings", span)),
            },
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem => {
                arith(op, &a, &b, span)
            }
            BinOp::And | BinOp::Or => Err(rt(Code::Internal, "unreachable", span)),
        }
    }

    // ------------------------------------------------------------ patterns

    pub(crate) fn match_pat(&self, p: &'p Pat, v: &Value<'p>, fr: &mut Frame<'p>) -> bool {
        match &p.kind {
            PatKind::Wild | PatKind::Unit => true,
            PatKind::Bind(slot) => {
                fr[*slot as usize] = v.clone();
                true
            }
            PatKind::Lit(l) => match (l, v) {
                (LitVal::Bool(a), Value::Bool(b)) => a == b,
                (LitVal::Float(a), Value::Float(b)) => a == b,
                (LitVal::Str(a), Value::Str(b)) => a.as_str() == &**b,
                (LitVal::Int(a), x) => x.to_big().is_some_and(|b| *a == b),
                _ => false,
            },
            PatKind::Ctor { variant, args, .. } => match v {
                Value::Adt {
                    variant: v2,
                    fields,
                    ..
                } if v2 == variant => args
                    .iter()
                    .zip(fields.iter())
                    .all(|(sp, sv)| self.match_pat(sp, sv, fr)),
                _ => false,
            },
            PatKind::Tuple(ps) => match v {
                Value::Tuple(vs) => ps
                    .iter()
                    .zip(vs.iter())
                    .all(|(sp, sv)| self.match_pat(sp, sv, fr)),
                _ => false,
            },
            PatKind::List { items, rest } => match v {
                Value::List(l) => {
                    let n = items.len();
                    let exact = rest.is_none();
                    if (exact && l.len() != n) || l.len() < n {
                        return false;
                    }
                    if !items
                        .iter()
                        .zip(l.as_slice())
                        .all(|(sp, sv)| self.match_pat(sp, sv, fr))
                    {
                        return false;
                    }
                    if let Some(Some(slot)) = rest {
                        fr[*slot as usize] = Value::List(l.slice(n, l.len()));
                    }
                    true
                }
                _ => false,
            },
        }
    }
}

fn arith<'p>(op: BinOp, a: &Value<'p>, b: &Value<'p>, span: ByteSpan) -> R<Value<'p>> {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => match op {
            BinOp::Add => Ok(Value::Float(x + y)),
            BinOp::Sub => Ok(Value::Float(x - y)),
            BinOp::Mul => Ok(Value::Float(x * y)),
            BinOp::Div => {
                if *y == 0.0 {
                    Err(rt(Code::FloatDomain, "division by 0.0", span))
                } else {
                    Ok(Value::Float(x / y))
                }
            }
            _ => Err(rt(Code::Internal, "bad float operator", span)),
        },
        (Value::Int(x), Value::Int(y)) => {
            let fast = match op {
                BinOp::Add => x.checked_add(*y),
                BinOp::Sub => x.checked_sub(*y),
                BinOp::Mul => x.checked_mul(*y),
                BinOp::Div | BinOp::Rem if *y == 0 => {
                    return Err(rt(Code::DivZero, "division by zero", span));
                }
                BinOp::Div => x.checked_div(*y),
                BinOp::Rem => x.checked_rem(*y),
                _ => None,
            };
            match fast {
                Some(v) => Ok(Value::Int(v)),
                None => big_arith(op, &BigInt::from(*x), &BigInt::from(*y), span),
            }
        }
        _ => match (a.to_big(), b.to_big()) {
            (Some(x), Some(y)) => big_arith(op, &x, &y, span),
            _ => Err(rt(Code::Internal, "bad arithmetic operands", span)),
        },
    }
}

fn big_arith<'p>(op: BinOp, x: &BigInt, y: &BigInt, span: ByteSpan) -> R<Value<'p>> {
    Ok(Value::from_big(match op {
        BinOp::Add => x + y,
        BinOp::Sub => x - y,
        BinOp::Mul => x * y,
        BinOp::Div | BinOp::Rem if y.is_zero() => {
            return Err(rt(Code::DivZero, "division by zero", span));
        }
        // num-bigint truncates toward zero, like Tez
        BinOp::Div => x / y,
        BinOp::Rem => x % y,
        _ => return Err(rt(Code::Internal, "bad integer operator", span)),
    }))
}

pub(crate) fn big_to_usize(v: &Value<'_>) -> Option<usize> {
    v.to_big()
        .filter(|b| !b.is_negative())
        .and_then(|b| b.to_usize())
}

impl<'p> Interp<'p> {
    /// Evaluates a top-level expression with a caller-provided frame.
    ///
    /// # Errors
    /// Returns the run-time failure, if any.
    pub fn eval_public(&mut self, e: &'p Expr, fr: &mut Vec<Value<'p>>) -> R<Value<'p>> {
        self.eval(e, fr)
    }
}
