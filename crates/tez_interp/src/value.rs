//! Runtime values.

use std::cmp::Ordering;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use tez_types::hir::{Expr, LocalId, Program, TypeId, TypeKind};

/// A slice of an immutable shared buffer; slicing is O(1).
#[derive(Clone, Debug)]
pub struct ListVal<'p> {
    pub(crate) buf: Rc<Vec<Value<'p>>>,
    pub(crate) start: usize,
    pub(crate) len: usize,
}

impl<'p> ListVal<'p> {
    /// Wraps a vector.
    #[must_use]
    pub fn new(v: Vec<Value<'p>>) -> Self {
        let len = v.len();
        Self {
            buf: Rc::new(v),
            start: 0,
            len,
        }
    }

    /// Number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// True when empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Elements as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Value<'p>] {
        &self.buf[self.start..self.start + self.len]
    }

    pub(crate) fn slice(&self, from: usize, to: usize) -> Self {
        Self {
            buf: Rc::clone(&self.buf),
            start: self.start + from,
            len: to - from,
        }
    }
}

/// A function value with its captured frame.
#[derive(Debug)]
pub struct Closure<'p> {
    pub(crate) params: &'p [LocalId],
    pub(crate) body: &'p Expr,
    pub(crate) captured: Vec<Value<'p>>,
}

/// A runtime value.
#[derive(Clone, Debug)]
pub enum Value<'p> {
    /// `()`
    Unit,
    /// Boolean.
    Bool(bool),
    /// Integer that fits in 64 bits (the fast path).
    Int(i64),
    /// Integer that does not.
    Big(Rc<BigInt>),
    /// Float.
    Float(f64),
    /// String.
    Str(Rc<str>),
    /// List.
    List(ListVal<'p>),
    /// Tuple.
    Tuple(Rc<Vec<Value<'p>>>),
    /// Record or sum-type value.
    Adt {
        /// Type.
        ty: TypeId,
        /// Variant index.
        variant: usize,
        /// Payload.
        fields: Rc<Vec<Value<'p>>>,
    },
    /// Lambda.
    Closure(Rc<Closure<'p>>),
    /// A named function used as a value.
    FnRef(usize),
}

impl Value<'_> {
    /// Normalizes a big integer to the small representation when it fits.
    #[must_use]
    pub fn from_big(b: BigInt) -> Self {
        b.to_i64()
            .map_or_else(|| Value::Big(Rc::new(b)), Value::Int)
    }

    /// Integer value as a big integer.
    #[must_use]
    pub fn to_big(&self) -> Option<BigInt> {
        match self {
            Value::Int(i) => Some(BigInt::from(*i)),
            Value::Big(b) => Some((**b).clone()),
            _ => None,
        }
    }
}

/// Structural equality; functions are never equal (the type checker forbids comparing them).
#[must_use]
pub fn values_eq(a: &Value<'_>, b: &Value<'_>) -> bool {
    match (a, b) {
        (Value::Unit, Value::Unit) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Big(x), Value::Big(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len()
                && x.as_slice()
                    .iter()
                    .zip(y.as_slice())
                    .all(|(p, q)| values_eq(p, q))
        }
        (Value::Tuple(x), Value::Tuple(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| values_eq(p, q))
        }
        (
            Value::Adt {
                ty: t1,
                variant: v1,
                fields: f1,
            },
            Value::Adt {
                ty: t2,
                variant: v2,
                fields: f2,
            },
        ) => {
            t1 == t2
                && v1 == v2
                && f1.len() == f2.len()
                && f1.iter().zip(f2.iter()).all(|(p, q)| values_eq(p, q))
        }
        _ => false,
    }
}

/// Ordering for Int, Float and Str.
#[must_use]
pub fn values_cmp(a: &Value<'_>, b: &Value<'_>) -> Option<Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
        (Value::Str(x), Value::Str(y)) => Some(x.cmp(y)),
        (x, y) => Some(x.to_big()?.cmp(&y.to_big()?)),
    }
}

/// Formats a float so it always reads as a float (`1.0`, not `1`).
#[must_use]
pub fn fmt_float(f: f64) -> String {
    let s = f.to_string();
    if f.is_finite() && !s.contains(['.', 'e', 'E']) {
        format!("{s}.0")
    } else {
        s
    }
}

impl Value<'_> {
    /// Debug rendering used by `tez run` and the tests.
    #[must_use]
    pub fn display(&self, prog: &Program) -> String {
        match self {
            Value::Unit => "()".into(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Big(b) => b.to_string(),
            Value::Float(f) => fmt_float(*f),
            Value::Str(s) => format!("{s:?}"),
            Value::List(l) => {
                format!(
                    "[{}]",
                    l.as_slice()
                        .iter()
                        .map(|v| v.display(prog))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Value::Tuple(vs) => {
                format!(
                    "({})",
                    vs.iter()
                        .map(|v| v.display(prog))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Value::Adt {
                ty,
                variant,
                fields,
            } => {
                let def = &prog.types[*ty];
                match &def.kind {
                    TypeKind::Record(fs) => {
                        let parts: Vec<String> = fs
                            .iter()
                            .zip(fields.iter())
                            .map(|(f, v)| format!("{}: {}", f.name, v.display(prog)))
                            .collect();
                        format!("{} {{ {} }}", def.name, parts.join(", "))
                    }
                    TypeKind::Adt(_) => {
                        let name = def.variant_name(*variant);
                        if fields.is_empty() {
                            name.to_string()
                        } else {
                            format!(
                                "{name}({})",
                                fields
                                    .iter()
                                    .map(|v| v.display(prog))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        }
                    }
                }
            }
            Value::Closure(_) | Value::FnRef(_) => "<function>".into(),
        }
    }
}
