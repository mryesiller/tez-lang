//! The checker: declaration collection, type resolution, then bodies.

mod expr;
mod finish;
mod pat;

use std::collections::{HashMap, HashSet};

use tez_diag::Code;
use tez_syntax::ast::{self, Ident, Item, TypeBody, TypeKind as AstTypeKind};
use tez_syntax::{ByteSpan, ParseOptions, Problem, parse};

use crate::PRELUDE_SRC;
use crate::hir::{
    self, FieldDef, FnDef, LocalId, ParamDef, Program, Refinement, TypeDef, VariantDef,
};
use crate::ty::{Ty, TyDisplay, TypeId};

/// Result of checking a file.
#[derive(Debug)]
pub struct CheckOutput {
    /// The program (prelude + user items). Meaningful only when `problems` is empty.
    pub program: Program,
    /// Everything wrong with the source, in source order.
    pub problems: Vec<Problem>,
}

/// Checks a parsed user file against the prelude.
///
/// `src` is the text of the user file (used to build fixes).
#[must_use]
pub fn check(file: &ast::File, src: &str) -> CheckOutput {
    let prelude = parse(PRELUDE_SRC, ParseOptions { prelude: true });
    let mut problems = Vec::new();
    if !prelude.problems.is_empty() {
        for p in &prelude.problems {
            problems.push(Problem::new(
                Code::Internal,
                format!("prelude does not parse: {}", p.message),
                ByteSpan::new(0, 0),
            ));
        }
        return CheckOutput {
            program: Program::default(),
            problems,
        };
    }
    let mut items: Vec<(&Item, bool)> = prelude.file.items.iter().map(|i| (i, true)).collect();
    items.extend(file.items.iter().map(|i| (i, false)));
    let mut ck = Checker::new(src);
    ck.run(&items);
    let mut problems = ck.problems;
    problems.sort_by_key(|p| p.span.start);
    CheckOutput {
        program: ck.program,
        problems,
    }
}

/// A refinement still in source form; lowered once scopes exist.
#[derive(Clone)]
pub(crate) struct RefineSrc<'a> {
    pub expr: &'a ast::Expr,
    /// Type of `it`.
    pub base: Ty,
    /// Came from an alias definition (already checked there; errors muted on re-check).
    pub from_alias: bool,
    pub tparams: Vec<String>,
}

pub(crate) struct Resolved<'a> {
    pub ty: Ty,
    pub refine: Vec<RefineSrc<'a>>,
}

enum TypeEntry {
    Adt(TypeId),
    Alias(usize),
}

enum AliasState<'a> {
    Unresolved,
    Resolving,
    Done(Ty, Vec<RefineSrc<'a>>),
    Failed,
}

struct AliasInfo<'a> {
    params: Vec<String>,
    body: &'a ast::TypeExpr,
    state: AliasState<'a>,
}

pub(crate) struct FnSrc<'a> {
    pub decl: &'a ast::FnDecl,
    pub param_refines: Vec<Vec<RefineSrc<'a>>>,
    pub ret_refines: Vec<RefineSrc<'a>>,
}

struct ConstSrc<'a> {
    decl: &'a ast::ConstDecl,
    refines: Vec<RefineSrc<'a>>,
}

pub(crate) enum Pending {
    Numeric {
        ty: Ty,
        span: ByteSpan,
        op: &'static str,
        int_only: bool,
    },
    Ordered {
        ty: Ty,
        span: ByteSpan,
    },
    Equatable {
        ty: Ty,
        span: ByteSpan,
    },
}

/// Type declarations whose predicates are lowered after all signatures exist.
struct TypePreds<'a> {
    type_id: TypeId,
    /// `(variant, field)` → refinement sources.
    fields: Vec<(usize, usize, Vec<RefineSrc<'a>>)>,
}

pub(crate) struct Checker<'a> {
    pub src: &'a str,
    pub problems: Vec<Problem>,
    muted: u32,
    pub program: Program,
    // namespaces
    type_names: HashMap<String, TypeEntry>,
    aliases: Vec<AliasInfo<'a>>,
    pub ctors: HashMap<String, (TypeId, usize)>,
    pub fn_names: HashMap<String, usize>,
    pub methods: HashMap<(String, String), usize>,
    pub const_names: HashMap<String, usize>,
    fn_srcs: Vec<Option<FnSrc<'a>>>,
    const_srcs: Vec<ConstSrc<'a>>,
    type_preds: Vec<TypePreds<'a>>,
    // per-frame inference state
    pub subst: Vec<Option<Ty>>,
    pub scopes: Vec<Vec<(String, LocalId, Ty)>>,
    pub next_local: u32,
    pub tparams: Vec<String>,
    pub pending: Vec<Pending>,
    pub loading_prelude: bool,
    pub in_const: Option<usize>,
    pub cur_calls: Vec<usize>,
    call_edges: Vec<Vec<usize>>,
    pub ambiguous_reported: bool,
}

const BUILTIN_TYPES: [&str; 6] = ["Int", "Float", "Bool", "Str", "Unit", "List"];
const MODULES: [&str; 6] = ["Int", "Float", "Str", "List", "Option", "Result"];

impl<'a> Checker<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            problems: Vec::new(),
            muted: 0,
            program: Program::default(),
            type_names: HashMap::new(),
            aliases: Vec::new(),
            ctors: HashMap::new(),
            fn_names: HashMap::new(),
            methods: HashMap::new(),
            const_names: HashMap::new(),
            fn_srcs: Vec::new(),
            const_srcs: Vec::new(),
            type_preds: Vec::new(),
            subst: Vec::new(),
            scopes: Vec::new(),
            next_local: 0,
            tparams: Vec::new(),
            pending: Vec::new(),
            loading_prelude: false,
            in_const: None,
            cur_calls: Vec::new(),
            call_edges: Vec::new(),
            ambiguous_reported: false,
        }
    }

    pub fn err(&mut self, p: Problem) {
        if self.muted == 0 && !self.loading_prelude {
            self.problems.push(p);
        } else if self.loading_prelude && self.muted == 0 {
            // a problem inside the prelude is a compiler bug
            let mut q = p;
            q.code = Code::Internal;
            q.message = format!("prelude: {}", q.message);
            q.span = ByteSpan::new(0, 0);
            self.problems.push(q);
        }
    }

    pub fn muted<R>(&mut self, on: bool, f: impl FnOnce(&mut Self) -> R) -> R {
        if on {
            self.muted += 1;
        }
        let r = f(self);
        if on {
            self.muted -= 1;
        }
        r
    }

    pub fn show(&self, ty: &Ty) -> String {
        let t = self.resolve(ty);
        TyDisplay {
            ty: &t,
            names: &|id| self.program.type_name(id),
        }
        .to_string()
    }

    // ------------------------------------------------------------ driver

    fn run(&mut self, items: &[(&'a Item, bool)]) {
        self.declare_types(items);
        self.resolve_type_bodies(items);
        self.declare_fns(items);
        self.declare_consts(items);
        self.check_consts();
        self.check_fns();
        self.check_tests(items);
        self.lower_type_preds();
        self.check_termination();
        self.check_patterns();
    }

    // ------------------------------------------------------------ stage 1: type names

    fn declare_types(&mut self, items: &[(&'a Item, bool)]) {
        for &(item, prelude) in items {
            self.loading_prelude = prelude;
            let Item::Type(td) = item else { continue };
            let name = &td.name;
            if self.name_taken(name, "type", prelude) {
                continue;
            }
            let params = self.dedup_params(&td.params);
            match &td.body {
                TypeBody::Alias(body) => {
                    let id = self.aliases.len();
                    self.aliases.push(AliasInfo {
                        params,
                        body,
                        state: AliasState::Unresolved,
                    });
                    self.type_names
                        .insert(name.name.clone(), TypeEntry::Alias(id));
                }
                body => {
                    let id = self.program.types.len();
                    let kind = match body {
                        TypeBody::Record(_) => hir::TypeKind::Record(Vec::new()),
                        _ => hir::TypeKind::Adt(Vec::new()),
                    };
                    self.program.types.push(TypeDef {
                        name: name.name.clone(),
                        params,
                        kind,
                        is_prelude: prelude,
                        span: td.span,
                    });
                    self.type_names
                        .insert(name.name.clone(), TypeEntry::Adt(id));
                    if let TypeBody::Adt(vs) = body {
                        for (vi, v) in vs.iter().enumerate() {
                            self.declare_ctor(&v.name, id, vi, prelude);
                        }
                    }
                }
            }
        }
        self.loading_prelude = false;
    }

    fn dedup_params(&mut self, params: &[Ident]) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for p in params {
            if !seen.insert(p.name.clone()) {
                self.err(Problem::new(
                    Code::NameDuplicate,
                    format!("type parameter `{}` is declared twice", p.name),
                    p.span,
                ));
            } else {
                out.push(p.name.clone());
            }
        }
        out
    }

    /// Reports duplicates / prelude clashes for a type name; true if the name is unusable.
    fn name_taken(&mut self, name: &Ident, what: &str, prelude: bool) -> bool {
        if !prelude && BUILTIN_TYPES.contains(&name.name.as_str()) {
            self.err(Problem::new(
                Code::NamePrelude,
                format!("`{}` is a built-in {what}", name.name),
                name.span,
            ));
            return true;
        }
        if let Some(entry) = self.type_names.get(&name.name) {
            let is_prelude_type =
                matches!(entry, TypeEntry::Adt(id) if self.program.types[*id].is_prelude);
            let code = if is_prelude_type && !prelude {
                Code::NamePrelude
            } else {
                Code::NameDuplicate
            };
            self.err(Problem::new(
                code,
                format!("{what} `{}` is already defined", name.name),
                name.span,
            ));
            return true;
        }
        false
    }

    fn declare_ctor(&mut self, name: &Ident, ty: TypeId, variant: usize, prelude: bool) {
        if let Some(&(other, _)) = self.ctors.get(&name.name) {
            let code = if self.program.types[other].is_prelude && !prelude {
                Code::NamePrelude
            } else {
                Code::NameDuplicate
            };
            self.err(Problem::new(
                code,
                format!("constructor `{}` is already defined", name.name),
                name.span,
            ));
        } else {
            self.ctors.insert(name.name.clone(), (ty, variant));
        }
    }

    // ------------------------------------------------------------ stage 2: type bodies

    fn resolve_type_bodies(&mut self, items: &[(&'a Item, bool)]) {
        for &(item, prelude) in items {
            self.loading_prelude = prelude;
            let Item::Type(td) = item else { continue };
            let Some(TypeEntry::Adt(id)) = self.type_names.get(&td.name.name) else {
                continue;
            };
            let id = *id;
            if self.program.types[id].span != td.span {
                continue; // a duplicate declaration that was rejected
            }
            self.tparams = self.program.types[id].params.clone();
            let mut preds = TypePreds {
                type_id: id,
                fields: Vec::new(),
            };
            let kind = match &td.body {
                TypeBody::Record(fields) => {
                    let mut seen = HashSet::new();
                    let mut out = Vec::new();
                    for (fi, f) in fields.iter().enumerate() {
                        if !seen.insert(f.name.name.clone()) {
                            self.err(Problem::new(
                                Code::NameDuplicate,
                                format!("field `{}` is declared twice", f.name.name),
                                f.name.span,
                            ));
                            continue;
                        }
                        let r = self.resolve_type(&f.ty, true);
                        preds.fields.push((0, fi, r.refine));
                        out.push(FieldDef {
                            name: f.name.name.clone(),
                            ty: r.ty,
                            refine: Vec::new(),
                        });
                    }
                    hir::TypeKind::Record(out)
                }
                TypeBody::Adt(variants) => {
                    let mut seen = HashSet::new();
                    let mut out = Vec::new();
                    for (vi, v) in variants.iter().enumerate() {
                        if !seen.insert(v.name.name.clone()) {
                            self.err(Problem::new(
                                Code::NameDuplicate,
                                format!("variant `{}` is declared twice", v.name.name),
                                v.name.span,
                            ));
                        }
                        let mut fields = Vec::new();
                        for (fi, t) in v.fields.iter().enumerate() {
                            let r = self.resolve_type(t, true);
                            preds.fields.push((vi, fi, r.refine));
                            fields.push(FieldDef {
                                name: fi.to_string(),
                                ty: r.ty,
                                refine: Vec::new(),
                            });
                        }
                        out.push(VariantDef {
                            name: v.name.name.clone(),
                            fields,
                        });
                    }
                    hir::TypeKind::Adt(out)
                }
                TypeBody::Alias(_) => continue,
            };
            self.program.types[id].kind = kind;
            self.type_preds.push(preds);
        }
        // resolve every alias once so errors inside unused aliases are reported
        for aid in 0..self.aliases.len() {
            self.loading_prelude = false;
            let _ = self.alias(aid);
        }
        self.tparams.clear();
        self.loading_prelude = false;
    }

    /// Resolves (once) an alias; `None` when it failed or is cyclic.
    fn alias(&mut self, aid: usize) -> Option<(Vec<String>, Ty, Vec<RefineSrc<'a>>)> {
        match &self.aliases[aid].state {
            AliasState::Done(t, r) => {
                return Some((self.aliases[aid].params.clone(), t.clone(), r.clone()));
            }
            AliasState::Failed => return None,
            AliasState::Resolving => {
                let span = self.aliases[aid].body.span;
                self.err(Problem::new(
                    Code::TypeRecursiveAlias,
                    "a type alias cannot refer to itself",
                    span,
                ));
                self.aliases[aid].state = AliasState::Failed;
                return None;
            }
            AliasState::Unresolved => {}
        }
        self.aliases[aid].state = AliasState::Resolving;
        let params = self.aliases[aid].params.clone();
        let body = self.aliases[aid].body;
        let saved = std::mem::replace(&mut self.tparams, params.clone());
        let r = self.resolve_type(body, true);
        self.tparams = saved;
        let refine: Vec<RefineSrc<'a>> = r
            .refine
            .into_iter()
            .map(|mut s| {
                s.from_alias = true;
                s
            })
            .collect();
        if matches!(self.aliases[aid].state, AliasState::Failed) {
            return None;
        }
        self.aliases[aid].state = AliasState::Done(r.ty.clone(), refine.clone());
        Some((params, r.ty, refine))
    }

    /// Resolves a written type. Refinements are allowed only at the top (`top == true`).
    pub fn resolve_type(&mut self, te: &'a ast::TypeExpr, top: bool) -> Resolved<'a> {
        let mut refine: Vec<RefineSrc<'a>> = Vec::new();
        let ty = match &te.kind {
            AstTypeKind::Unit => Ty::Unit,
            AstTypeKind::Tuple(ts) => {
                Ty::Tuple(ts.iter().map(|t| self.resolve_type(t, false).ty).collect())
            }
            AstTypeKind::Fn(ps, r) => {
                let ps = ps.iter().map(|t| self.resolve_type(t, false).ty).collect();
                Ty::func(ps, self.resolve_type(r, false).ty)
            }
            AstTypeKind::Named { name, args } => {
                let rargs: Vec<Ty> = args
                    .iter()
                    .map(|t| self.resolve_type(t, false).ty)
                    .collect();
                self.resolve_named(name, rargs, te.span, top, &mut refine)
            }
        };
        if let Some(pred) = &te.refine {
            if top {
                refine.push(RefineSrc {
                    expr: pred,
                    base: ty.clone(),
                    from_alias: false,
                    tparams: self.tparams.clone(),
                });
            } else {
                self.err(
                    Problem::new(
                        Code::TypeRefinementPosition,
                        "a refinement is only allowed at the top of a type",
                        pred.span,
                    )
                    .expected("a contract on the container, e.g. `requires xs.all(x => ..)`"),
                );
            }
        }
        Resolved { ty, refine }
    }

    fn arity_error(&mut self, name: &Ident, want: usize, got: usize) {
        self.err(
            Problem::new(
                Code::TypeArity,
                format!("`{}` takes {want} type argument(s), found {got}", name.name),
                name.span,
            )
            .expected(format!("{want} type argument(s)")),
        );
    }

    fn resolve_named(
        &mut self,
        name: &Ident,
        mut args: Vec<Ty>,
        span: ByteSpan,
        top: bool,
        refine: &mut Vec<RefineSrc<'a>>,
    ) -> Ty {
        let n = name.name.as_str();
        match n {
            "Int" | "Float" | "Bool" | "Str" | "Unit" => {
                let want = match n {
                    "Int" => Ty::Int,
                    "Float" => Ty::Float,
                    "Bool" => Ty::Bool,
                    "Str" => Ty::Str,
                    _ => Ty::Unit,
                };
                if !args.is_empty() {
                    self.arity_error(name, 0, args.len());
                    return Ty::Error;
                }
                return want;
            }
            "List" => {
                if args.len() == 1 {
                    return Ty::list(args.remove(0));
                }
                self.arity_error(name, 1, args.len());
                return Ty::Error;
            }
            _ => {}
        }
        if let Some(i) = self.tparams.iter().position(|p| p == n) {
            if !args.is_empty() {
                self.arity_error(name, 0, args.len());
            }
            return Ty::Param(self.tparams[i].clone());
        }
        match self.type_names.get(n) {
            Some(TypeEntry::Adt(id)) => {
                let id = *id;
                let want = self.program.types[id].params.len();
                if want != args.len() {
                    self.arity_error(name, want, args.len());
                    return Ty::Error;
                }
                Ty::Adt(id, args)
            }
            Some(TypeEntry::Alias(aid)) => {
                let aid = *aid;
                let Some((params, ty, arefine)) = self.alias(aid) else {
                    return Ty::Error;
                };
                if params.len() != args.len() {
                    self.arity_error(name, params.len(), args.len());
                    return Ty::Error;
                }
                let map: Vec<(String, Ty)> = params.into_iter().zip(args).collect();
                if !arefine.is_empty() {
                    if top {
                        for mut s in arefine {
                            s.base = s.base.subst_params(&map);
                            refine.push(s);
                        }
                    } else {
                        self.err(
                            Problem::new(
                                Code::TypeRefinementPosition,
                                format!("`{n}` is a refined type and cannot be nested here"),
                                span,
                            )
                            .expected("a contract on the container"),
                        );
                    }
                }
                ty.subst_params(&map)
            }
            None => {
                let cands: Vec<&str> = self.type_names.keys().map(String::as_str).collect();
                let mut p = Problem::new(
                    Code::NameUnknownType,
                    format!("unknown type `{n}`"),
                    name.span,
                );
                if let Some(s) = suggest(n, cands.into_iter()) {
                    p = p
                        .expected(format!("`{s}`"))
                        .fix(format!("use `{s}`"), name.span, s);
                } else {
                    p = p.expected("a declared type, or a type parameter listed in `[..]`");
                }
                self.err(p);
                Ty::Error
            }
        }
    }

    // ------------------------------------------------------------ stage 3: signatures

    fn declare_fns(&mut self, items: &[(&'a Item, bool)]) {
        for &(item, prelude) in items {
            self.loading_prelude = prelude;
            let Item::Fn(fd) = item else { continue };
            let id = self.program.fns.len();
            // duplicates
            let module = fd.module.as_ref().map(|m| m.name.clone());
            let dup = match &module {
                Some(m) => self
                    .methods
                    .contains_key(&(m.clone(), fd.name.name.clone())),
                None => {
                    self.fn_names.contains_key(&fd.name.name)
                        || self.const_names.contains_key(&fd.name.name)
                }
            };
            if dup {
                self.err(Problem::new(
                    Code::NameDuplicate,
                    format!("function `{}` is already defined", fd.name.name),
                    fd.name.span,
                ));
                self.fn_srcs.push(None);
                self.program.fns.push(placeholder_fn(fd, prelude));
                self.call_edges.push(Vec::new());
                continue;
            }
            match &module {
                Some(m) => {
                    self.methods.insert((m.clone(), fd.name.name.clone()), id);
                }
                None => {
                    self.fn_names.insert(fd.name.name.clone(), id);
                }
            }
            let tparams = self.dedup_params(&fd.tparams);
            self.tparams = tparams.clone();
            let mut seen = HashSet::new();
            let mut params = Vec::new();
            let mut param_refines = Vec::new();
            for p in &fd.params {
                if !seen.insert(p.name.name.clone()) {
                    self.err(Problem::new(
                        Code::NameDuplicate,
                        format!("parameter `{}` is declared twice", p.name.name),
                        p.name.span,
                    ));
                }
                let r = self.resolve_type(&p.ty, true);
                params.push(ParamDef {
                    name: p.name.name.clone(),
                    local: 0,
                    ty: r.ty,
                    refine: Vec::new(),
                });
                param_refines.push(r.refine);
            }
            let r = self.resolve_type(&fd.ret, true);
            let is_method = module.is_some() && params.first().is_some_and(|p| p.name == "self");
            self.program.fns.push(FnDef {
                module,
                name: fd.name.name.clone(),
                tparams,
                params,
                ret: r.ty,
                ret_refine: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                result_local: 0,
                decreases: None,
                body: None,
                is_extern: fd.is_extern,
                is_method,
                is_prelude: prelude,
                is_recursive: false,
                nlocals: 0,
                span: fd.span,
            });
            self.fn_srcs.push(Some(FnSrc {
                decl: fd,
                param_refines,
                ret_refines: r.refine,
            }));
            self.call_edges.push(Vec::new());
        }
        self.tparams.clear();
        self.loading_prelude = false;
    }

    fn declare_consts(&mut self, items: &[(&'a Item, bool)]) {
        for &(item, _) in items {
            let Item::Const(cd) = item else { continue };
            if self.const_names.contains_key(&cd.name.name)
                || self.fn_names.contains_key(&cd.name.name)
            {
                self.err(Problem::new(
                    Code::NameDuplicate,
                    format!("`{}` is already defined", cd.name.name),
                    cd.name.span,
                ));
                continue;
            }
            let r = self.resolve_type(&cd.ty, true);
            let id = self.program.consts.len();
            self.const_names.insert(cd.name.name.clone(), id);
            self.program.consts.push(hir::ConstDef {
                name: cd.name.name.clone(),
                ty: r.ty,
                refine: Vec::new(),
                value: unit_expr(cd.span),
                nlocals: 0,
                span: cd.span,
            });
            self.const_srcs.push(ConstSrc {
                decl: cd,
                refines: r.refine,
            });
        }
    }

    // ------------------------------------------------------------ frames

    pub fn begin_frame(&mut self, tparams: Vec<String>) {
        self.subst.clear();
        self.scopes = vec![Vec::new()];
        self.next_local = 0;
        self.tparams = tparams;
        self.pending.clear();
        self.cur_calls.clear();
        self.ambiguous_reported = false;
    }

    pub fn fresh(&mut self) -> Ty {
        self.subst.push(None);
        Ty::Var(u32::try_from(self.subst.len() - 1).unwrap_or(u32::MAX))
    }

    pub fn new_local(&mut self) -> LocalId {
        let id = self.next_local;
        self.next_local += 1;
        id
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn lookup_local(&self, name: &str) -> Option<(LocalId, Ty)> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|s| s.iter().rev())
            .find(|(n, _, _)| n == name)
            .map(|(_, id, t)| (*id, t.clone()))
    }

    /// Declares a binding, reporting shadowing.
    pub fn declare_local(&mut self, name: &Ident, ty: Ty) -> LocalId {
        let n = name.name.as_str();
        if self.lookup_local(n).is_some() {
            self.err(
                Problem::new(
                    Code::NameShadowing,
                    format!("`{n}` is already in scope"),
                    name.span,
                )
                .expected("a new name"),
            );
        } else if !self.loading_prelude
            && (self.fn_names.contains_key(n) || self.const_names.contains_key(n))
        {
            self.err(
                Problem::new(
                    Code::NameShadowing,
                    format!("`{n}` would hide the top-level `{n}`"),
                    name.span,
                )
                .expected("a new name"),
            );
        }
        let id = self.new_local();
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name.name.clone(), id, ty));
        }
        id
    }

    /// Declares `it` / `result`.
    pub fn declare_special(&mut self, name: &str, ty: Ty, span: ByteSpan) -> LocalId {
        if self.lookup_local(name).is_some() {
            self.err(
                Problem::new(
                    Code::NameShadowing,
                    format!("`{name}` is already in scope here"),
                    span,
                )
                .expected("rename the conflicting binding"),
            );
        }
        let id = self.new_local();
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name.to_string(), id, ty));
        }
        id
    }

    /// Lowers refinement sources inside the current frame.
    pub fn lower_refines(&mut self, srcs: &[RefineSrc<'a>]) -> Vec<Refinement> {
        let mut out = Vec::new();
        for s in srcs {
            let saved = std::mem::replace(&mut self.tparams, s.tparams.clone());
            self.push_scope();
            let it = self.declare_special("it", s.base.clone(), s.expr.span);
            let pred = self.muted(s.from_alias, |c| c.expr(s.expr, Some(&Ty::Bool)));
            self.pop_scope();
            self.tparams = saved;
            out.push(Refinement {
                it,
                pred,
                nlocals: 0,
            });
        }
        out
    }

    // ------------------------------------------------------------ stage 4

    fn check_consts(&mut self) {
        for i in 0..self.const_srcs.len() {
            let decl = self.const_srcs[i].decl;
            let refines = self.const_srcs[i].refines.clone();
            let ty = self.program.consts[i].ty.clone();
            self.begin_frame(Vec::new());
            self.in_const = Some(i);
            let mut value = self.expr(&decl.value, Some(&ty));
            let mut rs = self.lower_refines(&refines);
            self.finish_frame_exprs(
                std::iter::once(&mut value).chain(rs.iter_mut().map(|r| &mut r.pred)),
            );
            self.in_const = None;
            let nl = self.next_local;
            for r in &mut rs {
                r.nlocals = nl;
            }
            let c = &mut self.program.consts[i];
            c.value = value;
            c.refine = rs;
            c.nlocals = nl;
        }
    }

    fn check_fns(&mut self) {
        for id in 0..self.program.fns.len() {
            let prelude = self.program.fns[id].is_prelude;
            self.loading_prelude = prelude;
            if self.fn_srcs[id].is_none() {
                continue;
            }
            self.check_fn(id);
        }
        self.loading_prelude = false;
    }

    fn check_fn(&mut self, id: usize) {
        let (decl, param_refines, ret_refines) = {
            let Some(s) = &self.fn_srcs[id] else { return };
            (s.decl, s.param_refines.clone(), s.ret_refines.clone())
        };
        let tparams = self.program.fns[id].tparams.clone();
        self.begin_frame(tparams);
        let param_tys: Vec<Ty> = self.program.fns[id]
            .params
            .iter()
            .map(|p| p.ty.clone())
            .collect();
        let ret = self.program.fns[id].ret.clone();

        let mut params = Vec::new();
        for (i, p) in decl.params.iter().enumerate() {
            let ty = param_tys.get(i).cloned().unwrap_or(Ty::Error);
            let local = self.declare_local(&p.name, ty.clone());
            let refine = self.lower_refines(&param_refines[i]);
            params.push((local, refine));
        }
        let mut requires: Vec<hir::Expr> = decl
            .requires
            .iter()
            .map(|e| self.expr(e, Some(&Ty::Bool)))
            .collect();
        let ret_span = decl.ret.span;
        self.push_scope();
        let result_local = self.declare_special("result", ret.clone(), ret_span);
        let mut ensures: Vec<hir::Expr> = decl
            .ensures
            .iter()
            .map(|e| self.expr(e, Some(&Ty::Bool)))
            .collect();
        self.pop_scope();
        let mut ret_refine = self.lower_refines(&ret_refines);
        let mut decreases = decl
            .decreases
            .as_ref()
            .map(|e| self.expr(e, Some(&Ty::Int)));
        let mut body = decl.body.as_ref().map(|b| self.expr(b, Some(&ret)));

        // finalize types
        {
            let mut exprs: Vec<&mut hir::Expr> = Vec::new();
            exprs.extend(requires.iter_mut());
            exprs.extend(ensures.iter_mut());
            exprs.extend(decreases.iter_mut());
            exprs.extend(body.iter_mut());
            exprs.extend(ret_refine.iter_mut().map(|r| &mut r.pred));
            for (_, rs) in &mut params {
                exprs.extend(rs.iter_mut().map(|r| &mut r.pred));
            }
            self.finish_frame_exprs(exprs.into_iter());
        }
        let nl = self.next_local;
        let calls = std::mem::take(&mut self.cur_calls);
        self.call_edges[id] = calls;
        let f = &mut self.program.fns[id];
        for (p, (local, refine)) in f.params.iter_mut().zip(params) {
            p.local = local;
            p.refine = refine;
        }
        f.requires = requires;
        f.ensures = ensures;
        f.result_local = result_local;
        f.ret_refine = ret_refine;
        f.decreases = decreases;
        f.body = body;
        f.nlocals = nl;
    }

    fn check_tests(&mut self, items: &[(&'a Item, bool)]) {
        let mut seen = HashSet::new();
        for &(item, _) in items {
            let Item::Test(td) = item else { continue };
            if !seen.insert(td.name.name.clone()) {
                self.err(Problem::new(
                    Code::NameDuplicate,
                    format!("test `{}` is already defined", td.name.name),
                    td.name.span,
                ));
                continue;
            }
            self.begin_frame(Vec::new());
            let mut body = self.expr(&td.body, Some(&Ty::Bool));
            self.finish_frame_exprs(std::iter::once(&mut body));
            self.program.tests.push(hir::TestDef {
                name: td.name.name.clone(),
                body,
                nlocals: self.next_local,
                span: td.span,
            });
        }
    }

    fn lower_type_preds(&mut self) {
        let preds = std::mem::take(&mut self.type_preds);
        for tp in preds {
            let params = self.program.types[tp.type_id].params.clone();
            self.loading_prelude = self.program.types[tp.type_id].is_prelude;
            for (vi, fi, srcs) in tp.fields {
                if srcs.is_empty() {
                    continue;
                }
                let mut lowered = Vec::new();
                for s in &srcs {
                    self.begin_frame(params.clone());
                    let mut r = self.lower_refines(std::slice::from_ref(s));
                    self.finish_frame_exprs(r.iter_mut().map(|x| &mut x.pred));
                    let nl = self.next_local;
                    for x in &mut r {
                        x.nlocals = nl;
                    }
                    lowered.extend(r);
                }
                let t = &mut self.program.types[tp.type_id];
                let fields = match &mut t.kind {
                    hir::TypeKind::Record(fs) => Some(fs),
                    hir::TypeKind::Adt(vs) => vs.get_mut(vi).map(|v| &mut v.fields),
                };
                if let Some(f) = fields.and_then(|fs| fs.get_mut(fi)) {
                    f.refine = lowered;
                }
            }
        }
        self.loading_prelude = false;
    }

    // ------------------------------------------------------------ misc shared helpers

    pub fn module_of(&self, ty: &Ty) -> Option<&'static str> {
        let t = self.resolve(ty);
        match t {
            Ty::Int => Some("Int"),
            Ty::Float => Some("Float"),
            Ty::Str => Some("Str"),
            Ty::List(_) => Some("List"),
            Ty::Adt(id, _) => match self.program.types[id].name.as_str() {
                "Option" if self.program.types[id].is_prelude => Some("Option"),
                "Result" if self.program.types[id].is_prelude => Some("Result"),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn is_module(name: &str) -> bool {
        MODULES.contains(&name)
    }

    pub fn adt_has_fn(&self, id: TypeId, args: &[Ty], depth: u32) -> bool {
        if depth > 8 {
            return false;
        }
        let t = &self.program.types[id];
        let map: Vec<(String, Ty)> = t.params.iter().cloned().zip(args.iter().cloned()).collect();
        let fields: Vec<&FieldDef> = match &t.kind {
            hir::TypeKind::Record(fs) => fs.iter().collect(),
            hir::TypeKind::Adt(vs) => vs.iter().flat_map(|v| v.fields.iter()).collect(),
        };
        fields.iter().any(|f| {
            f.ty.subst_params(&map)
                .contains_fn(&|i, a| self.adt_has_fn(i, a, depth + 1))
        })
    }
}

fn unit_expr(span: ByteSpan) -> hir::Expr {
    hir::Expr {
        kind: hir::ExprKind::Unit,
        ty: Ty::Unit,
        span,
    }
}

fn placeholder_fn(fd: &ast::FnDecl, prelude: bool) -> FnDef {
    FnDef {
        module: fd.module.as_ref().map(|m| m.name.clone()),
        name: fd.name.name.clone(),
        tparams: Vec::new(),
        params: Vec::new(),
        ret: Ty::Error,
        ret_refine: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        result_local: 0,
        decreases: None,
        body: None,
        is_extern: fd.is_extern,
        is_method: false,
        is_prelude: prelude,
        is_recursive: false,
        nlocals: 0,
        span: fd.span,
    }
}

/// The closest candidate within a small edit distance.
pub(crate) fn suggest<'s>(name: &str, cands: impl Iterator<Item = &'s str>) -> Option<String> {
    let limit = if name.len() <= 3 { 1 } else { 2 };
    cands
        .filter(|c| *c != name)
        .map(|c| (edit_distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, c)| (*d, c.len()))
        .map(|(_, c)| c.to_string())
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}
