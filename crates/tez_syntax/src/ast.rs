//! Syntax tree produced by the parser. Every node carries a [`ByteSpan`].

use crate::source::ByteSpan;

/// A name with its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ident {
    /// The text.
    pub name: String,
    /// Where it appears.
    pub span: ByteSpan,
}

/// A whole source file.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct File {
    /// Top-level items in source order.
    pub items: Vec<Item>,
}

/// A top-level declaration.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Item {
    /// `type ..`
    Type(TypeDecl),
    /// `fn ..` or `extern fn ..`
    Fn(FnDecl),
    /// `const ..`
    Const(ConstDecl),
    /// `test ..`
    Test(TestDecl),
    /// `use "file.tez"`
    Use(UseDecl),
}

impl Item {
    /// Range of the whole item.
    #[must_use]
    pub fn span(&self) -> ByteSpan {
        match self {
            Item::Type(d) => d.span,
            Item::Fn(d) => d.span,
            Item::Const(d) => d.span,
            Item::Test(d) => d.span,
            Item::Use(d) => d.span,
        }
    }
}

/// `use "relative/path.tez"`: the items of that file become part of this program.
#[derive(Debug, Clone, PartialEq)]
pub struct UseDecl {
    /// The path as written (relative to the importing file).
    pub path: String,
    /// Whole declaration.
    pub span: ByteSpan,
}

/// `type Name[Params] = body`
#[derive(Debug, Clone, PartialEq)]
pub struct TypeDecl {
    /// Type name.
    pub name: Ident,
    /// Type parameters.
    pub params: Vec<Ident>,
    /// Definition.
    pub body: TypeBody,
    /// Whole declaration.
    pub span: ByteSpan,
}

/// Right-hand side of a type declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeBody {
    /// `{ x: Int, y: Int }`
    Record(Vec<FieldDecl>),
    /// `A | B(Int)`
    Adt(Vec<Variant>),
    /// `Int where it > 0`, `List[Int]`, ..
    Alias(TypeExpr),
}

/// A record field declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldDecl {
    /// Field name.
    pub name: Ident,
    /// Field type.
    pub ty: TypeExpr,
}

/// A sum-type variant.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    /// Constructor name.
    pub name: Ident,
    /// Positional payload types.
    pub fields: Vec<TypeExpr>,
    /// Whole variant.
    pub span: ByteSpan,
}

/// A type as written, possibly refined with `where`.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeExpr {
    /// Shape.
    pub kind: TypeKind,
    /// `where` predicate over `it`.
    pub refine: Option<Box<Expr>>,
    /// Whole type.
    pub span: ByteSpan,
}

/// Shape of a written type.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    /// `Name` or `Name[Args]`
    Named {
        /// Type name.
        name: Ident,
        /// Type arguments.
        args: Vec<TypeExpr>,
    },
    /// `(A, B)`
    Tuple(Vec<TypeExpr>),
    /// `(A, B) -> C`
    Fn(Vec<TypeExpr>, Box<TypeExpr>),
    /// `()`
    Unit,
}

/// A function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// Name.
    pub name: Ident,
    /// Declared type.
    pub ty: TypeExpr,
}

/// A function declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct FnDecl {
    /// Module for prelude methods (`List` in `List.map`).
    pub module: Option<Ident>,
    /// Function name.
    pub name: Ident,
    /// Type parameters.
    pub tparams: Vec<Ident>,
    /// Parameters.
    pub params: Vec<Param>,
    /// Return type.
    pub ret: TypeExpr,
    /// `requires` clauses.
    pub requires: Vec<Expr>,
    /// `ensures` clauses.
    pub ensures: Vec<Expr>,
    /// `decreases` measure.
    pub decreases: Option<Expr>,
    /// Body; `None` for `extern fn`.
    pub body: Option<Expr>,
    /// True for `extern fn` (prelude only).
    pub is_extern: bool,
    /// Whole declaration.
    pub span: ByteSpan,
}

/// `const name: T = expr`
#[derive(Debug, Clone, PartialEq)]
pub struct ConstDecl {
    /// Name.
    pub name: Ident,
    /// Declared type.
    pub ty: TypeExpr,
    /// Value.
    pub value: Expr,
    /// Whole declaration.
    pub span: ByteSpan,
}

/// `test name = expr`
#[derive(Debug, Clone, PartialEq)]
pub struct TestDecl {
    /// Name.
    pub name: Ident,
    /// Boolean expression.
    pub body: Expr,
    /// Whole declaration.
    pub span: ByteSpan,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    /// `||`
    Or,
    /// `&&`
    And,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `++`
    Concat,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
}

impl BinOp {
    /// Source spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            BinOp::Or => "||",
            BinOp::And => "&&",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Concat => "++",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
        }
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    /// `-x`
    Neg,
    /// `!x`
    Not,
}

/// An expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// Shape.
    pub kind: ExprKind,
    /// Source range.
    pub span: ByteSpan,
}

/// Expression shapes.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// Integer literal (digits).
    Int(String),
    /// Float literal.
    Float(String),
    /// String literal (decoded).
    Str(String),
    /// `true` / `false`
    Bool(bool),
    /// `()`
    Unit,
    /// Lowercase name: local, parameter, constant or function.
    Var(Ident),
    /// Uppercase name: constructor.
    Ctor(Ident),
    /// `Module.name`: static built-in.
    Path {
        /// Module (`List`).
        module: Ident,
        /// Member (`range`).
        name: Ident,
    },
    /// `[a, b]`
    List(Vec<Expr>),
    /// `(a, b)`
    Tuple(Vec<Expr>),
    /// `(e)`
    Paren(Box<Expr>),
    /// `Name { f: e, .. }`
    Record {
        /// Record type name.
        ty: Ident,
        /// Field initialisers.
        fields: Vec<FieldInit>,
    },
    /// `base with { f: e }`
    With {
        /// Record being copied.
        base: Box<Expr>,
        /// Replaced fields.
        fields: Vec<FieldInit>,
    },
    /// `-e`, `!e`
    Unary(UnOp, Box<Expr>),
    /// `l op r`
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `f(args)`
    Call {
        /// Callee.
        callee: Box<Expr>,
        /// Arguments.
        args: Vec<Expr>,
    },
    /// `recv.name(args)`
    MethodCall {
        /// Receiver.
        recv: Box<Expr>,
        /// Method name.
        name: Ident,
        /// Arguments after the receiver.
        args: Vec<Expr>,
    },
    /// `e.field`
    Field(Box<Expr>, Ident),
    /// `e.0`
    TupleField(Box<Expr>, u32),
    /// `e[i]`
    Index(Box<Expr>, Box<Expr>),
    /// `x => e`, `(a, b) => e`
    Lambda {
        /// Parameters.
        params: Vec<LambdaParam>,
        /// Body.
        body: Box<Expr>,
    },
    /// `if c { a } else { b }`; `els` is `None` only after a reported error.
    If {
        /// Condition.
        cond: Box<Expr>,
        /// Then branch.
        then: Box<Expr>,
        /// Else branch.
        els: Option<Box<Expr>>,
    },
    /// `match e { arms }`
    Match {
        /// Scrutinee.
        scrut: Box<Expr>,
        /// Arms in order.
        arms: Vec<Arm>,
    },
    /// `{ stmts; tail }`
    Block {
        /// `let` and `assert` statements.
        stmts: Vec<Stmt>,
        /// Final expression.
        tail: Box<Expr>,
    },
}

/// `field: expr` in a record literal or update.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldInit {
    /// Field name.
    pub name: Ident,
    /// Value.
    pub value: Expr,
}

/// A lambda parameter with an optional annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct LambdaParam {
    /// Name.
    pub name: Ident,
    /// Optional type.
    pub ty: Option<TypeExpr>,
}

/// A match arm.
#[derive(Debug, Clone, PartialEq)]
pub struct Arm {
    /// Pattern.
    pub pat: Pat,
    /// Optional guard.
    pub guard: Option<Expr>,
    /// Result.
    pub body: Expr,
    /// Whole arm.
    pub span: ByteSpan,
}

/// A block statement.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `let pat: T = init`
    Let {
        /// Pattern.
        pat: Pat,
        /// Optional annotation.
        ty: Option<TypeExpr>,
        /// Initialiser.
        init: Expr,
        /// Whole statement.
        span: ByteSpan,
    },
    /// `assert cond`
    Assert {
        /// Condition.
        cond: Expr,
        /// Whole statement.
        span: ByteSpan,
    },
}

/// A literal inside a pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum Lit {
    /// Integer; may start with `-`.
    Int(String),
    /// Float; may start with `-`.
    Float(String),
    /// String.
    Str(String),
    /// Boolean.
    Bool(bool),
}

/// A pattern.
#[derive(Debug, Clone, PartialEq)]
pub struct Pat {
    /// Shape.
    pub kind: PatKind,
    /// Source range.
    pub span: ByteSpan,
}

/// Pattern shapes.
#[derive(Debug, Clone, PartialEq)]
pub enum PatKind {
    /// `_`
    Wild,
    /// `()`
    Unit,
    /// A name binding.
    Bind(Ident),
    /// A literal.
    Lit(Lit),
    /// `Name` or `Name(p, ..)`
    Ctor {
        /// Constructor.
        name: Ident,
        /// Sub-patterns.
        args: Vec<Pat>,
    },
    /// `(a, b)`
    Tuple(Vec<Pat>),
    /// `[a, b, ..rest]`
    List {
        /// Leading elements.
        items: Vec<Pat>,
        /// `Some(None)` for `..`, `Some(Some(x))` for `..x`, `None` for a closed list.
        rest: Option<Option<Ident>>,
    },
}
