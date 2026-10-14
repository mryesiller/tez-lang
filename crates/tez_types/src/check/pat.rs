//! Pattern checking.

use num_bigint::BigInt;
use tez_diag::Code;
use tez_syntax::Problem;
use tez_syntax::ast::{self, Lit, PatKind as A};

use super::{Checker, suggest};
use crate::hir::{LitVal, Pat, PatKind as H};
use crate::ty::Ty;

impl<'a> Checker<'a> {
    /// Checks `p` against `ty`, declaring its bindings in the current scope.
    pub fn pattern(&mut self, p: &'a ast::Pat, ty: &Ty) -> Pat {
        let mut binds = Vec::new();
        self.pat_inner(p, ty, &mut binds)
    }

    fn pat_inner(&mut self, p: &'a ast::Pat, ty: &Ty, binds: &mut Vec<String>) -> Pat {
        let span = p.span;
        let mk = |kind: H, ty: &Ty| Pat {
            kind,
            ty: ty.clone(),
            span,
        };
        match &p.kind {
            A::Wild => mk(H::Wild, ty),
            A::Unit => {
                self.expect_ty(span, ty, &Ty::Unit);
                mk(H::Unit, ty)
            }
            A::Bind(id) => {
                if binds.contains(&id.name) {
                    self.err(Problem::new(
                        Code::MatchBindingMismatch,
                        format!("`{}` is bound twice in this pattern", id.name),
                        id.span,
                    ));
                } else {
                    binds.push(id.name.clone());
                }
                let local = self.declare_local(id, ty.clone());
                mk(H::Bind(local), ty)
            }
            A::Lit(l) => {
                let (val, lt) = match l {
                    Lit::Int(s) => (
                        LitVal::Int(s.parse::<BigInt>().unwrap_or_default()),
                        Ty::Int,
                    ),
                    Lit::Float(s) => (LitVal::Float(s.parse::<f64>().unwrap_or(0.0)), Ty::Float),
                    Lit::Str(s) => (LitVal::Str(s.clone()), Ty::Str),
                    Lit::Bool(b) => (LitVal::Bool(*b), Ty::Bool),
                };
                self.expect_ty(span, ty, &lt);
                mk(H::Lit(val), ty)
            }
            A::Ctor { name, args } => {
                let Some(&(tid, variant)) = self.ctors.get(&name.name) else {
                    let mut pr = Problem::new(
                        Code::NameUnknownCtor,
                        format!("unknown constructor `{}`", name.name),
                        name.span,
                    );
                    if let Some(s) = suggest(&name.name, self.ctors.keys().map(String::as_str)) {
                        pr = pr
                            .expected(format!("`{s}`"))
                            .fix(format!("use `{s}`"), name.span, s);
                    }
                    self.err(pr);
                    for a in args {
                        self.pat_inner(a, &Ty::Error, binds);
                    }
                    return mk(H::Wild, &Ty::Error);
                };
                let nparams = self.program.types[tid].params.len();
                let targs: Vec<Ty> = (0..nparams).map(|_| self.fresh()).collect();
                let adt = Ty::Adt(tid, targs.clone());
                let ok = self.expect_ty(span, ty, &adt);
                let map: Vec<(String, Ty)> = self.program.types[tid]
                    .params
                    .iter()
                    .cloned()
                    .zip(targs)
                    .collect();
                let ftys: Vec<Ty> = self.program.types[tid]
                    .variant_fields(variant)
                    .map(|fs| fs.iter().map(|f| f.ty.subst_params(&map)).collect())
                    .unwrap_or_default();
                if ftys.len() != args.len() {
                    self.err(
                        Problem::new(
                            Code::TypeArity,
                            format!(
                                "constructor `{}` has {} field(s), the pattern has {}",
                                name.name,
                                ftys.len(),
                                args.len()
                            ),
                            span,
                        )
                        .expected(format!("{} sub-pattern(s)", ftys.len())),
                    );
                    for a in args {
                        self.pat_inner(a, &Ty::Error, binds);
                    }
                    return mk(H::Wild, &adt);
                }
                let sub: Vec<Pat> = args
                    .iter()
                    .zip(&ftys)
                    .map(|(a, t)| self.pat_inner(a, if ok { t } else { &Ty::Error }, binds))
                    .collect();
                Pat {
                    kind: H::Ctor {
                        ty: tid,
                        variant,
                        args: sub,
                    },
                    ty: ty.clone(),
                    span,
                }
            }
            A::Tuple(ps) => {
                let elems: Vec<Ty> = ps.iter().map(|_| self.fresh()).collect();
                let ok = self.expect_ty(span, ty, &Ty::Tuple(elems.clone()));
                let sub: Vec<Pat> = ps
                    .iter()
                    .zip(&elems)
                    .map(|(a, t)| self.pat_inner(a, if ok { t } else { &Ty::Error }, binds))
                    .collect();
                mk(H::Tuple(sub), ty)
            }
            A::List { items, rest } => {
                let elem = self.fresh();
                let ok = self.expect_ty(span, ty, &Ty::list(elem.clone()));
                let et = if ok { elem.clone() } else { Ty::Error };
                let sub: Vec<Pat> = items
                    .iter()
                    .map(|a| self.pat_inner(a, &et, binds))
                    .collect();
                let rest_local = match rest {
                    None => None,
                    Some(None) => Some(None),
                    Some(Some(id)) => {
                        if binds.contains(&id.name) {
                            self.err(Problem::new(
                                Code::MatchBindingMismatch,
                                format!("`{}` is bound twice in this pattern", id.name),
                                id.span,
                            ));
                        } else {
                            binds.push(id.name.clone());
                        }
                        let rt = if ok { Ty::list(elem) } else { Ty::Error };
                        Some(Some(self.declare_local(id, rt)))
                    }
                };
                mk(
                    H::List {
                        items: sub,
                        rest: rest_local,
                    },
                    ty,
                )
            }
        }
    }
}
