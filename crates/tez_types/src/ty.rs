//! Types as the checker sees them.

use std::fmt;

/// Index of a type definition in [`crate::hir::Program::types`].
pub type TypeId = usize;

/// A type. Refinements are not part of `Ty`; they attach to declarations (see `hir`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// Arbitrary-precision integer.
    Int,
    /// 64-bit float.
    Float,
    /// Boolean.
    Bool,
    /// Unicode string.
    Str,
    /// The unit type.
    Unit,
    /// `List[T]`
    List(Box<Ty>),
    /// `(A, B, ..)`, two or more elements.
    Tuple(Vec<Ty>),
    /// `(A, B) -> C`
    Fn(Vec<Ty>, Box<Ty>),
    /// A declared record or sum type with its type arguments.
    Adt(TypeId, Vec<Ty>),
    /// A type parameter of the enclosing declaration.
    Param(String),
    /// An inference variable.
    Var(u32),
    /// A type that already produced an error; unifies with anything silently.
    Error,
}

impl Ty {
    /// `List[elem]`.
    #[must_use]
    pub fn list(elem: Ty) -> Ty {
        Ty::List(Box::new(elem))
    }

    /// `(params) -> ret`.
    #[must_use]
    pub fn func(params: Vec<Ty>, ret: Ty) -> Ty {
        Ty::Fn(params, Box::new(ret))
    }

    /// True if the type mentions an inference variable.
    #[must_use]
    pub fn has_var(&self) -> bool {
        match self {
            Ty::Var(_) => true,
            Ty::List(t) => t.has_var(),
            Ty::Tuple(ts) | Ty::Adt(_, ts) => ts.iter().any(Ty::has_var),
            Ty::Fn(ps, r) => ps.iter().any(Ty::has_var) || r.has_var(),
            _ => false,
        }
    }

    /// True if the type mentions [`Ty::Error`].
    #[must_use]
    pub fn has_error(&self) -> bool {
        match self {
            Ty::Error => true,
            Ty::List(t) => t.has_error(),
            Ty::Tuple(ts) | Ty::Adt(_, ts) => ts.iter().any(Ty::has_error),
            Ty::Fn(ps, r) => ps.iter().any(Ty::has_error) || r.has_error(),
            _ => false,
        }
    }

    /// True if values of this type can contain functions (and so cannot be compared).
    #[must_use]
    pub fn contains_fn(&self, adt_has_fn: &dyn Fn(TypeId, &[Ty]) -> bool) -> bool {
        match self {
            Ty::Fn(..) => true,
            Ty::List(t) => t.contains_fn(adt_has_fn),
            Ty::Tuple(ts) => ts.iter().any(|t| t.contains_fn(adt_has_fn)),
            Ty::Adt(id, args) => adt_has_fn(*id, args),
            _ => false,
        }
    }

    /// Replaces type parameters by name.
    #[must_use]
    pub fn subst_params(&self, map: &[(String, Ty)]) -> Ty {
        match self {
            Ty::Param(n) => map
                .iter()
                .find(|(m, _)| m == n)
                .map_or_else(|| self.clone(), |(_, t)| t.clone()),
            Ty::List(t) => Ty::list(t.subst_params(map)),
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| t.subst_params(map)).collect()),
            Ty::Adt(id, ts) => Ty::Adt(*id, ts.iter().map(|t| t.subst_params(map)).collect()),
            Ty::Fn(ps, r) => Ty::func(
                ps.iter().map(|t| t.subst_params(map)).collect(),
                r.subst_params(map),
            ),
            other => other.clone(),
        }
    }

    /// Applies `f` to every node, bottom-up.
    #[must_use]
    pub fn map(&self, f: &mut dyn FnMut(&Ty) -> Option<Ty>) -> Ty {
        if let Some(t) = f(self) {
            return t;
        }
        match self {
            Ty::List(t) => Ty::list(t.map(f)),
            Ty::Tuple(ts) => Ty::Tuple(ts.iter().map(|t| t.map(f)).collect()),
            Ty::Adt(id, ts) => Ty::Adt(*id, ts.iter().map(|t| t.map(f)).collect()),
            Ty::Fn(ps, r) => Ty::func(ps.iter().map(|t| t.map(f)).collect(), r.map(f)),
            other => other.clone(),
        }
    }
}

/// Renders types in Tez syntax; ADT names come from `names`.
pub struct TyDisplay<'a> {
    /// The type to print.
    pub ty: &'a Ty,
    /// Name of each type id.
    pub names: &'a dyn Fn(TypeId) -> String,
}

impl fmt::Display for TyDisplay<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let show = |t: &Ty| {
            TyDisplay {
                ty: t,
                names: self.names,
            }
            .to_string()
        };
        match self.ty {
            Ty::Int => write!(f, "Int"),
            Ty::Float => write!(f, "Float"),
            Ty::Bool => write!(f, "Bool"),
            Ty::Str => write!(f, "Str"),
            Ty::Unit => write!(f, "()"),
            Ty::List(t) => write!(f, "List[{}]", show(t)),
            Ty::Tuple(ts) => write!(
                f,
                "({})",
                ts.iter().map(show).collect::<Vec<_>>().join(", ")
            ),
            Ty::Fn(ps, r) => {
                write!(
                    f,
                    "({}) -> {}",
                    ps.iter().map(show).collect::<Vec<_>>().join(", "),
                    show(r)
                )
            }
            Ty::Adt(id, args) if args.is_empty() => write!(f, "{}", (self.names)(*id)),
            Ty::Adt(id, args) => write!(
                f,
                "{}[{}]",
                (self.names)(*id),
                args.iter().map(show).collect::<Vec<_>>().join(", ")
            ),
            Ty::Param(n) => write!(f, "{n}"),
            Ty::Var(_) => write!(f, "_"),
            Ty::Error => write!(f, "?"),
        }
    }
}
