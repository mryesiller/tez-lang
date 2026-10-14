//! Typed, name-resolved intermediate representation shared by the interpreter,
//! the verifier and the code generator.
//!
//! Method calls are desugared to direct calls, names are resolved to ids, and every
//! expression carries its final type.

use num_bigint::BigInt;
use tez_syntax::ByteSpan;

pub use crate::ty::{Ty, TypeId};

/// Index into [`Program::fns`].
pub type FnId = usize;
/// Index into [`Program::consts`].
pub type ConstId = usize;
/// Slot of a local variable within one function (or refinement) frame.
pub type LocalId = u32;

/// A checked program: prelude items first, then user items.
#[derive(Debug, Clone, Default)]
pub struct Program {
    /// Record and sum types.
    pub types: Vec<TypeDef>,
    /// Functions, including prelude functions.
    pub fns: Vec<FnDef>,
    /// Constants, in dependency order.
    pub consts: Vec<ConstDef>,
    /// `test` items.
    pub tests: Vec<TestDef>,
}

impl Program {
    /// Finds a user or prelude function by its (qualified) name.
    #[must_use]
    pub fn find_fn(&self, name: &str) -> Option<FnId> {
        self.fns.iter().position(|f| f.full_name() == name)
    }

    /// Display name of a type id.
    #[must_use]
    pub fn type_name(&self, id: TypeId) -> String {
        self.types
            .get(id)
            .map_or_else(|| "?".into(), |t| t.name.clone())
    }

    /// Renders a type in Tez syntax.
    #[must_use]
    pub fn show_ty(&self, ty: &Ty) -> String {
        crate::ty::TyDisplay {
            ty,
            names: &|id| self.type_name(id),
        }
        .to_string()
    }
}

/// `where` predicate over the value named by `it`.
#[derive(Debug, Clone)]
pub struct Refinement {
    /// Local bound to the refined value.
    pub it: LocalId,
    /// `Bool` predicate.
    pub pred: Expr,
    /// Number of local slots the predicate needs (for type-level predicates, which own their frame).
    pub nlocals: u32,
}

/// A record or sum type.
#[derive(Debug, Clone)]
pub struct TypeDef {
    /// Declared name.
    pub name: String,
    /// Type parameters.
    pub params: Vec<String>,
    /// Shape.
    pub kind: TypeKind,
    /// Declared in the prelude.
    pub is_prelude: bool,
    /// Declaration site.
    pub span: ByteSpan,
}

/// Shape of a [`TypeDef`].
#[derive(Debug, Clone)]
pub enum TypeKind {
    /// A record; also stored as a single variant with named fields.
    Record(Vec<FieldDef>),
    /// A sum type.
    Adt(Vec<VariantDef>),
}

/// A record field.
#[derive(Debug, Clone)]
pub struct FieldDef {
    /// Field name.
    pub name: String,
    /// Field type.
    pub ty: Ty,
    /// Refinements on the field.
    pub refine: Vec<Refinement>,
}

/// A sum-type variant.
#[derive(Debug, Clone)]
pub struct VariantDef {
    /// Constructor name.
    pub name: String,
    /// Positional payload.
    pub fields: Vec<FieldDef>,
}

impl TypeDef {
    /// Field/payload definitions of variant `variant` (a record has the single variant 0).
    #[must_use]
    pub fn variant_fields(&self, variant: usize) -> Option<&[FieldDef]> {
        match &self.kind {
            TypeKind::Record(fs) if variant == 0 => Some(fs),
            TypeKind::Adt(vs) => vs.get(variant).map(|v| v.fields.as_slice()),
            TypeKind::Record(_) => None,
        }
    }

    /// Number of variants (1 for records).
    #[must_use]
    pub fn variant_count(&self) -> usize {
        match &self.kind {
            TypeKind::Record(_) => 1,
            TypeKind::Adt(vs) => vs.len(),
        }
    }

    /// Name of variant `variant` (the type name for records).
    #[must_use]
    pub fn variant_name(&self, variant: usize) -> &str {
        match &self.kind {
            TypeKind::Record(_) => &self.name,
            TypeKind::Adt(vs) => vs.get(variant).map_or("?", |v| v.name.as_str()),
        }
    }
}

/// A function parameter.
#[derive(Debug, Clone)]
pub struct ParamDef {
    /// Name.
    pub name: String,
    /// Frame slot.
    pub local: LocalId,
    /// Type.
    pub ty: Ty,
    /// Refinements (may mention earlier parameters).
    pub refine: Vec<Refinement>,
}

/// Name and type of a local slot (for counterexamples and diagnostics).
#[derive(Debug, Clone)]
pub struct LocalInfo {
    /// Source name; empty for anonymous slots (`_`, `it`).
    pub name: String,
    /// Final type.
    pub ty: Ty,
}

/// A function.
#[derive(Debug, Clone)]
pub struct FnDef {
    /// Module for prelude methods (`List` for `List.map`).
    pub module: Option<String>,
    /// Unqualified name.
    pub name: String,
    /// Type parameters.
    pub tparams: Vec<String>,
    /// Parameters.
    pub params: Vec<ParamDef>,
    /// Return type.
    pub ret: Ty,
    /// Refinements on the result (`it` bound to the result).
    pub ret_refine: Vec<Refinement>,
    /// Preconditions.
    pub requires: Vec<Expr>,
    /// Postconditions; `result` is [`FnDef::result_local`].
    pub ensures: Vec<Expr>,
    /// Slot bound to the result inside `ensures`.
    pub result_local: LocalId,
    /// Termination measure.
    pub decreases: Option<Expr>,
    /// Body; `None` for `extern fn`.
    pub body: Option<Expr>,
    /// Implemented by the runtime.
    pub is_extern: bool,
    /// First parameter is `self`: callable with method syntax.
    pub is_method: bool,
    /// Declared in the prelude.
    pub is_prelude: bool,
    /// Part of a call cycle (needs `decreases`).
    pub is_recursive: bool,
    /// Number of local slots in the frame.
    pub nlocals: u32,
    /// Name and type of every local slot.
    pub locals: Vec<LocalInfo>,
    /// Declaration site.
    pub span: ByteSpan,
}

impl FnDef {
    /// `List.map` for methods, the bare name otherwise.
    #[must_use]
    pub fn full_name(&self) -> String {
        match &self.module {
            Some(m) => format!("{m}.{}", self.name),
            None => self.name.clone(),
        }
    }
}

/// A constant.
#[derive(Debug, Clone)]
pub struct ConstDef {
    /// Name.
    pub name: String,
    /// Type.
    pub ty: Ty,
    /// Refinements on the value.
    pub refine: Vec<Refinement>,
    /// Initialiser.
    pub value: Expr,
    /// Frame slots needed by the initialiser.
    pub nlocals: u32,
    /// Declaration site.
    pub span: ByteSpan,
}

/// A `test` item.
#[derive(Debug, Clone)]
pub struct TestDef {
    /// Name.
    pub name: String,
    /// Boolean body.
    pub body: Expr,
    /// Frame slots needed.
    pub nlocals: u32,
    /// Declaration site.
    pub span: ByteSpan,
}

/// Unary operators.
pub use tez_syntax::ast::{BinOp, UnOp};

/// A typed expression.
#[derive(Debug, Clone)]
pub struct Expr {
    /// Shape.
    pub kind: ExprKind,
    /// Final type.
    pub ty: Ty,
    /// Source range.
    pub span: ByteSpan,
}

/// Expression shapes.
#[derive(Debug, Clone)]
pub enum ExprKind {
    /// Integer.
    Int(BigInt),
    /// Float.
    Float(f64),
    /// String.
    Str(String),
    /// Boolean.
    Bool(bool),
    /// `()`
    Unit,
    /// Local variable.
    Local(LocalId),
    /// Constant.
    Const(ConstId),
    /// A named function used as a value; `targs` instantiate its type parameters.
    FnRef(FnId, Vec<Ty>),
    /// Direct call of a known function.
    Call {
        /// Callee.
        f: FnId,
        /// Instantiation of the callee's type parameters.
        targs: Vec<Ty>,
        /// Arguments (receiver first for methods).
        args: Vec<Expr>,
    },
    /// Call of a function-typed value.
    CallValue {
        /// Callee expression.
        callee: Box<Expr>,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// Constructor application (records use variant 0, fields in declaration order).
    Ctor {
        /// Type.
        ty: TypeId,
        /// Variant index.
        variant: usize,
        /// Type arguments.
        targs: Vec<Ty>,
        /// Payload.
        args: Vec<Expr>,
    },
    /// Record field read.
    Field(Box<Expr>, usize),
    /// Tuple element read.
    TupleField(Box<Expr>, usize),
    /// Record update: `(field index, new value)` pairs.
    With(Box<Expr>, Vec<(usize, Expr)>),
    /// Tuple.
    Tuple(Vec<Expr>),
    /// List literal.
    List(Vec<Expr>),
    /// `list[i]`
    Index(Box<Expr>, Box<Expr>),
    /// Unary operation.
    Unary(UnOp, Box<Expr>),
    /// Binary operation; operand type is `lhs.ty`.
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `if`
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    /// `match`
    Match(Box<Expr>, Vec<Arm>),
    /// Block.
    Block(Vec<Stmt>, Box<Expr>),
    /// Lambda.
    Lambda {
        /// Parameter slots.
        params: Vec<LocalId>,
        /// Body.
        body: Box<Expr>,
    },
}

/// A match arm.
#[derive(Debug, Clone)]
pub struct Arm {
    /// Pattern.
    pub pat: Pat,
    /// Guard.
    pub guard: Option<Expr>,
    /// Result.
    pub body: Expr,
}

/// A block statement.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// `let pat = init`; `refine` comes from an annotated refined type.
    Let {
        /// Pattern.
        pat: Pat,
        /// Initialiser.
        init: Expr,
        /// Refinements of the annotation.
        refine: Vec<Refinement>,
        /// Site.
        span: ByteSpan,
    },
    /// `assert cond`
    Assert(Expr),
}

/// Literal value in a pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum LitVal {
    /// Integer.
    Int(BigInt),
    /// Float.
    Float(f64),
    /// String.
    Str(String),
    /// Boolean.
    Bool(bool),
}

/// A typed pattern.
#[derive(Debug, Clone)]
pub struct Pat {
    /// Shape.
    pub kind: PatKind,
    /// Type of the matched value.
    pub ty: Ty,
    /// Source range.
    pub span: ByteSpan,
}

/// Pattern shapes.
#[derive(Debug, Clone)]
pub enum PatKind {
    /// `_`
    Wild,
    /// `()`
    Unit,
    /// Binds the value to a slot.
    Bind(LocalId),
    /// Literal.
    Lit(LitVal),
    /// Constructor.
    Ctor {
        /// Type.
        ty: TypeId,
        /// Variant.
        variant: usize,
        /// Sub-patterns.
        args: Vec<Pat>,
    },
    /// Tuple.
    Tuple(Vec<Pat>),
    /// List: leading items and an optional rest (`Some(Some(slot))` binds the rest).
    List {
        /// Leading elements.
        items: Vec<Pat>,
        /// `None` = exact length; `Some(slot)` = at least `items.len()`.
        rest: Option<Option<LocalId>>,
    },
}

impl Expr {
    /// Calls `f` on every type stored in this expression tree (including patterns and
    /// refinement predicates), with the span of the node that holds it.
    pub fn visit_tys_mut(&mut self, f: &mut dyn FnMut(&mut Ty, ByteSpan)) {
        let span = self.span;
        f(&mut self.ty, span);
        match &mut self.kind {
            ExprKind::FnRef(_, ts) => ts.iter_mut().for_each(|t| f(t, span)),
            ExprKind::Call { targs, args, .. } | ExprKind::Ctor { targs, args, .. } => {
                targs.iter_mut().for_each(|t| f(t, span));
                args.iter_mut().for_each(|a| a.visit_tys_mut(f));
            }
            ExprKind::CallValue { callee, args } => {
                callee.visit_tys_mut(f);
                args.iter_mut().for_each(|a| a.visit_tys_mut(f));
            }
            ExprKind::Field(b, _) | ExprKind::TupleField(b, _) | ExprKind::Unary(_, b) => {
                b.visit_tys_mut(f)
            }
            ExprKind::With(b, ups) => {
                b.visit_tys_mut(f);
                ups.iter_mut().for_each(|(_, e)| e.visit_tys_mut(f));
            }
            ExprKind::Tuple(es) | ExprKind::List(es) => {
                es.iter_mut().for_each(|e| e.visit_tys_mut(f))
            }
            ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) => {
                a.visit_tys_mut(f);
                b.visit_tys_mut(f);
            }
            ExprKind::If(c, t, e) => {
                c.visit_tys_mut(f);
                t.visit_tys_mut(f);
                e.visit_tys_mut(f);
            }
            ExprKind::Match(s, arms) => {
                s.visit_tys_mut(f);
                for a in arms {
                    a.pat.visit_tys_mut(f);
                    if let Some(g) = &mut a.guard {
                        g.visit_tys_mut(f);
                    }
                    a.body.visit_tys_mut(f);
                }
            }
            ExprKind::Block(stmts, tail) => {
                for s in stmts {
                    match s {
                        Stmt::Let {
                            pat, init, refine, ..
                        } => {
                            pat.visit_tys_mut(f);
                            init.visit_tys_mut(f);
                            refine.iter_mut().for_each(|r| r.pred.visit_tys_mut(f));
                        }
                        Stmt::Assert(e) => e.visit_tys_mut(f),
                    }
                }
                tail.visit_tys_mut(f);
            }
            ExprKind::Lambda { body, .. } => body.visit_tys_mut(f),
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Str(_)
            | ExprKind::Bool(_)
            | ExprKind::Unit
            | ExprKind::Local(_)
            | ExprKind::Const(_) => {}
        }
    }

    /// Visits this expression and every sub-expression, parents first.
    pub fn walk(&self, f: &mut dyn FnMut(&Expr)) {
        f(self);
        match &self.kind {
            ExprKind::Call { args, .. } | ExprKind::Ctor { args, .. } => {
                args.iter().for_each(|a| a.walk(f))
            }
            ExprKind::CallValue { callee, args } => {
                callee.walk(f);
                args.iter().for_each(|a| a.walk(f));
            }
            ExprKind::Field(b, _) | ExprKind::TupleField(b, _) | ExprKind::Unary(_, b) => b.walk(f),
            ExprKind::With(b, ups) => {
                b.walk(f);
                ups.iter().for_each(|(_, e)| e.walk(f));
            }
            ExprKind::Tuple(es) | ExprKind::List(es) => es.iter().for_each(|e| e.walk(f)),
            ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) => {
                a.walk(f);
                b.walk(f);
            }
            ExprKind::If(c, t, e) => {
                c.walk(f);
                t.walk(f);
                e.walk(f);
            }
            ExprKind::Match(s, arms) => {
                s.walk(f);
                for a in arms {
                    if let Some(g) = &a.guard {
                        g.walk(f);
                    }
                    a.body.walk(f);
                }
            }
            ExprKind::Block(stmts, tail) => {
                for s in stmts {
                    match s {
                        Stmt::Let { init, refine, .. } => {
                            init.walk(f);
                            refine.iter().for_each(|r| r.pred.walk(f));
                        }
                        Stmt::Assert(e) => e.walk(f),
                    }
                }
                tail.walk(f);
            }
            ExprKind::Lambda { body, .. } => body.walk(f),
            _ => {}
        }
    }
}

impl Pat {
    /// Calls `f` on every type stored in this pattern tree.
    pub fn visit_tys_mut(&mut self, f: &mut dyn FnMut(&mut Ty, ByteSpan)) {
        let span = self.span;
        f(&mut self.ty, span);
        match &mut self.kind {
            PatKind::Ctor { args, .. } => args.iter_mut().for_each(|p| p.visit_tys_mut(f)),
            PatKind::Tuple(ps) => ps.iter_mut().for_each(|p| p.visit_tys_mut(f)),
            PatKind::List { items, .. } => items.iter_mut().for_each(|p| p.visit_tys_mut(f)),
            PatKind::Wild | PatKind::Unit | PatKind::Bind(_) | PatKind::Lit(_) => {}
        }
    }
}
