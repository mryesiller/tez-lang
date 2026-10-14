//! Match exhaustiveness and reachability (Maranget's usefulness algorithm).

use num_bigint::BigInt;
use tez_syntax::ByteSpan;

use crate::hir::{LitVal, Pat, PatKind, Program, TypeKind};
use crate::ty::{Ty, TypeId};

#[derive(Debug, Clone, PartialEq)]
enum LitKey {
    Int(BigInt),
    Float(u64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
enum Ctor {
    Variant(TypeId, usize),
    Tuple(usize),
    Nil,
    Cons,
    Bool(bool),
    Unit,
    Lit(LitKey),
}

#[derive(Debug, Clone)]
enum CPat {
    Wild,
    Ctor(Ctor, Vec<CPat>),
}

/// What is wrong with a `match`.
pub(crate) enum Issue {
    /// Some value is not covered; carries a missing pattern.
    NonExhaustive(String),
    /// An arm is shadowed by earlier arms.
    Unreachable,
}

/// One arm as seen by the checker.
pub(crate) struct ArmInfo<'p> {
    pub pat: &'p Pat,
    pub guarded: bool,
}

fn lower(p: &Pat) -> CPat {
    match &p.kind {
        PatKind::Wild | PatKind::Bind(_) => CPat::Wild,
        PatKind::Unit => CPat::Ctor(Ctor::Unit, Vec::new()),
        PatKind::Lit(l) => CPat::Ctor(
            match l {
                LitVal::Bool(b) => Ctor::Bool(*b),
                LitVal::Int(i) => Ctor::Lit(LitKey::Int(i.clone())),
                LitVal::Float(f) => Ctor::Lit(LitKey::Float(f.to_bits())),
                LitVal::Str(s) => Ctor::Lit(LitKey::Str(s.clone())),
            },
            Vec::new(),
        ),
        PatKind::Ctor { ty, variant, args } => CPat::Ctor(
            Ctor::Variant(*ty, *variant),
            args.iter().map(lower).collect(),
        ),
        PatKind::Tuple(ps) => CPat::Ctor(Ctor::Tuple(ps.len()), ps.iter().map(lower).collect()),
        PatKind::List { items, rest } => {
            let mut tail = if rest.is_none() {
                CPat::Ctor(Ctor::Nil, Vec::new())
            } else {
                CPat::Wild
            };
            for it in items.iter().rev() {
                tail = CPat::Ctor(Ctor::Cons, vec![lower(it), tail]);
            }
            tail
        }
    }
}

/// Complete constructor signature of `ty` with sub-types, or `None` if infinite/unknown.
fn signature(prog: &Program, ty: &Ty) -> Option<Vec<(Ctor, Vec<Ty>)>> {
    match ty {
        Ty::Bool => Some(vec![
            (Ctor::Bool(false), vec![]),
            (Ctor::Bool(true), vec![]),
        ]),
        Ty::Unit => Some(vec![(Ctor::Unit, vec![])]),
        Ty::Tuple(ts) => Some(vec![(Ctor::Tuple(ts.len()), ts.clone())]),
        Ty::List(e) => Some(vec![
            (Ctor::Nil, vec![]),
            (Ctor::Cons, vec![(**e).clone(), ty.clone()]),
        ]),
        Ty::Adt(id, args) => {
            let def = prog.types.get(*id)?;
            let map: Vec<(String, Ty)> = def
                .params
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect();
            let n = match &def.kind {
                TypeKind::Record(_) => 1,
                TypeKind::Adt(vs) => vs.len(),
            };
            Some(
                (0..n)
                    .map(|i| {
                        let subs = def
                            .variant_fields(i)
                            .map(|fs| fs.iter().map(|f| f.ty.subst_params(&map)).collect())
                            .unwrap_or_default();
                        (Ctor::Variant(*id, i), subs)
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

fn sub_types(prog: &Program, ty: &Ty, c: &Ctor, arity: usize) -> Vec<Ty> {
    signature(prog, ty)
        .and_then(|sig| sig.into_iter().find(|(k, _)| k == c).map(|(_, t)| t))
        .unwrap_or_else(|| vec![Ty::Error; arity])
}

fn wilds(n: usize) -> Vec<CPat> {
    vec![CPat::Wild; n]
}

fn specialize(rows: &[Vec<CPat>], c: &Ctor, arity: usize) -> Vec<Vec<CPat>> {
    let mut out = Vec::new();
    for row in rows {
        match &row[0] {
            CPat::Ctor(k, args) if k == c => {
                let mut r = args.clone();
                r.extend_from_slice(&row[1..]);
                out.push(r);
            }
            CPat::Wild => {
                let mut r = wilds(arity);
                r.extend_from_slice(&row[1..]);
                out.push(r);
            }
            CPat::Ctor(..) => {}
        }
    }
    out
}

fn rebuild(c: &Ctor, arity: usize, mut w: Vec<CPat>) -> Vec<CPat> {
    let rest = w.split_off(arity.min(w.len()));
    let mut out = vec![CPat::Ctor(c.clone(), w)];
    out.extend(rest);
    out
}

fn useful(prog: &Program, rows: &[Vec<CPat>], q: &[CPat], tys: &[Ty]) -> Option<Vec<CPat>> {
    if q.is_empty() {
        return rows.is_empty().then(Vec::new);
    }
    let ty = &tys[0];
    match &q[0] {
        CPat::Ctor(c, args) => {
            let arity = args.len();
            let subs = sub_types(prog, ty, c, arity);
            let mut q2 = args.clone();
            q2.extend_from_slice(&q[1..]);
            let mut tys2 = subs;
            tys2.resize(arity, Ty::Error);
            tys2.extend_from_slice(&tys[1..]);
            let w = useful(prog, &specialize(rows, c, arity), &q2, &tys2)?;
            Some(rebuild(c, arity, w))
        }
        CPat::Wild => {
            let mut heads: Vec<Ctor> = Vec::new();
            for r in rows {
                if let CPat::Ctor(c, _) = &r[0]
                    && !heads.contains(c)
                {
                    heads.push(c.clone());
                }
            }
            let sig = signature(prog, ty);
            if let Some(sig) = &sig
                && sig.iter().all(|(c, _)| heads.contains(c))
            {
                for (c, subs) in sig {
                    let arity = subs.len();
                    let mut q2 = wilds(arity);
                    q2.extend_from_slice(&q[1..]);
                    let mut tys2 = subs.clone();
                    tys2.extend_from_slice(&tys[1..]);
                    if let Some(w) = useful(prog, &specialize(rows, c, arity), &q2, &tys2) {
                        return Some(rebuild(c, arity, w));
                    }
                }
                return None;
            }
            let default: Vec<Vec<CPat>> = rows
                .iter()
                .filter(|r| matches!(r[0], CPat::Wild))
                .map(|r| r[1..].to_vec())
                .collect();
            let w = useful(prog, &default, &q[1..], &tys[1..])?;
            let head = sig
                .and_then(|s| s.into_iter().find(|(c, _)| !heads.contains(c)))
                .map_or(CPat::Wild, |(c, subs)| CPat::Ctor(c, wilds(subs.len())));
            let mut out = vec![head];
            out.extend(w);
            Some(out)
        }
    }
}

fn show(prog: &Program, p: &CPat) -> String {
    match p {
        CPat::Wild => "_".into(),
        CPat::Ctor(c, args) => match c {
            Ctor::Variant(t, v) => {
                let name = prog.types.get(*t).map_or("?", |d| d.variant_name(*v));
                if args.is_empty() {
                    name.to_string()
                } else {
                    format!(
                        "{name}({})",
                        args.iter()
                            .map(|a| show(prog, a))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
            Ctor::Tuple(_) => format!(
                "({})",
                args.iter()
                    .map(|a| show(prog, a))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Ctor::Nil => "[]".into(),
            Ctor::Cons => {
                let mut items = Vec::new();
                let mut cur = p;
                loop {
                    match cur {
                        CPat::Ctor(Ctor::Cons, a) if a.len() == 2 => {
                            items.push(show(prog, &a[0]));
                            cur = &a[1];
                        }
                        CPat::Ctor(Ctor::Nil, _) => return format!("[{}]", items.join(", ")),
                        _ => {
                            items.push("..".into());
                            return format!("[{}]", items.join(", "));
                        }
                    }
                }
            }
            Ctor::Bool(b) => b.to_string(),
            Ctor::Unit => "()".into(),
            Ctor::Lit(LitKey::Int(i)) => i.to_string(),
            Ctor::Lit(LitKey::Float(b)) => f64::from_bits(*b).to_string(),
            Ctor::Lit(LitKey::Str(s)) => format!("{s:?}"),
        },
    }
}

/// Checks the arms of a `match` on a value of type `scrut`.
/// Returns `(arm index, issue)`; the index is `None` for whole-match issues.
pub(crate) fn check_arms(
    prog: &Program,
    scrut: &Ty,
    arms: &[ArmInfo<'_>],
) -> Vec<(Option<usize>, Issue)> {
    if scrut.has_error() || scrut.has_var() || arms.iter().any(|a| a.pat.ty.has_error()) {
        return Vec::new();
    }
    let tys = [scrut.clone()];
    let mut out = Vec::new();
    let mut rows: Vec<Vec<CPat>> = Vec::new();
    for (i, a) in arms.iter().enumerate() {
        let p = lower(a.pat);
        if useful(prog, &rows, std::slice::from_ref(&p), &tys).is_none() {
            out.push((Some(i), Issue::Unreachable));
        }
        if !a.guarded {
            rows.push(vec![p]);
        }
    }
    if let Some(w) = useful(prog, &rows, &[CPat::Wild], &tys) {
        out.push((
            None,
            Issue::NonExhaustive(w.first().map_or_else(|| "_".into(), |p| show(prog, p))),
        ));
    }
    out
}

/// For `let` patterns: the missing pattern if the pattern is refutable.
pub(crate) fn check_irrefutable(prog: &Program, pat: &Pat) -> Option<String> {
    if pat.ty.has_error() || pat.ty.has_var() {
        return None;
    }
    let rows = vec![vec![lower(pat)]];
    useful(prog, &rows, &[CPat::Wild], std::slice::from_ref(&pat.ty))
        .map(|w| w.first().map_or_else(|| "_".into(), |p| show(prog, p)))
}

#[allow(dead_code)]
fn _span_marker(_: ByteSpan) {}
