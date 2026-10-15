//! The code generator. See the crate docs for the value representation.

use std::collections::{BTreeSet, HashMap};

use num_bigint::BigInt;
use num_traits::ToPrimitive;
use serde_json::{Value as Json, json};
use tez_types::Ty;
use tez_types::hir::{
    BinOp, Expr, ExprKind, FnId, LitVal, Pat, PatKind, Program, Stmt, TypeKind, UnOp,
};
use wasm_encoder::{
    AbstractHeapType, BlockType, CodeSection, CompositeInnerType, CompositeType, ConstExpr,
    CustomSection, ElementSection, Elements, EntityType, ExportKind, ExportSection, FieldType,
    Function, FunctionSection, GlobalSection, GlobalType, HeapType, ImportSection,
    Instruction as I, Module as WModule, RefType, StartSection, StorageType, StructType, SubType,
    TypeSection, ValType,
};

/// Why code generation failed.
#[derive(Debug, Clone)]
pub struct CompileError(pub String);

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A compiled module.
#[derive(Debug, Clone)]
pub struct Module {
    /// The WebAssembly binary (with a `tez-meta` custom section for the runtime).
    pub wasm: Vec<u8>,
}

const ANY: ValType = ValType::Ref(RefType::ANYREF);
const I31: HeapType = HeapType::Abstract {
    shared: false,
    ty: AbstractHeapType::I31,
};
const FIT_MIN: i64 = -(1 << 30);
const FIT_MAX: i64 = 1 << 30;

fn concrete(idx: u32) -> HeapType {
    HeapType::Concrete(idx)
}

fn ref_to(idx: u32) -> ValType {
    ValType::Ref(RefType {
        nullable: false,
        heap_type: concrete(idx),
    })
}

fn ref_null_to(idx: u32) -> ValType {
    ValType::Ref(RefType {
        nullable: true,
        heap_type: concrete(idx),
    })
}

/// Signatures of imported functions (all other values are `anyref`).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Sig {
    /// `(anyref × n) -> anyref`
    Any(u32),
    /// `(anyref × n) -> i32`
    AnyToI32(u32),
    /// `(i64) -> anyref`
    I64Any,
    /// `(i32) -> anyref`
    I32Any,
    /// `(anyref, i32) -> anyref`
    AnyI32Any,
    /// `(i32) -> ()`
    FailI32,
    /// `(f64) -> anyref`
    F64Any,
    /// `(anyref) -> f64`
    AnyF64,
}

fn fixed_imports() -> Vec<(&'static str, Sig)> {
    vec![
        ("big_from_i64", Sig::I64Any),
        ("big_add", Sig::Any(2)),
        ("big_sub", Sig::Any(2)),
        ("big_mul", Sig::Any(2)),
        ("big_div", Sig::Any(2)),
        ("big_rem", Sig::Any(2)),
        ("big_neg", Sig::Any(1)),
        ("big_lt", Sig::AnyToI32(2)),
        ("big_le", Sig::AnyToI32(2)),
        ("big_eq", Sig::AnyToI32(2)),
        ("big_lit", Sig::I32Any),
        ("str_lit", Sig::I32Any),
        ("str_eq", Sig::AnyToI32(2)),
        ("str_lt", Sig::AnyToI32(2)),
        ("str_le", Sig::AnyToI32(2)),
        ("str_concat", Sig::Any(2)),
        ("list_new", Sig::Any(0)),
        ("list_push", Sig::Any(2)),
        ("list_len", Sig::AnyToI32(1)),
        ("list_get", Sig::AnyI32Any),
        ("list_slice", Sig::AnyI32Any),
        ("deep_eq", Sig::AnyToI32(2)),
        ("fail", Sig::FailI32),
    ]
}

/// Indices of the integer helper functions.
#[derive(Clone, Copy, Default)]
struct IntHelpers {
    add: u32,
    sub: u32,
    mul: u32,
    div: u32,
    rem: u32,
    neg: u32,
    lt: u32,
    le: u32,
    eq: u32,
}

const N_INT_HELPERS: u32 = 9;

struct FnState {
    code: Vec<I<'static>>,
    extra: Vec<ValType>,
    nparams: u32,
    slots: HashMap<u32, u32>,
    depth: u32,
    /// Compiling a user function or lambda body (result type `anyref`): calls may become tail calls.
    tail_ok: bool,
    /// Indices in `code` of `call` / `call_ref` instructions that may be tail calls.
    tail_cands: Vec<usize>,
}

impl FnState {
    fn new(nparams: u32) -> Self {
        Self {
            code: Vec::new(),
            extra: Vec::new(),
            nparams,
            slots: HashMap::new(),
            depth: 0,
            tail_ok: false,
            tail_cands: Vec::new(),
        }
    }

    /// True when nothing but block ends follows instruction `i` (so its value is the function's result).
    fn only_ends_after(&self, i: usize) -> bool {
        let mut j = i + 1;
        while j < self.code.len() {
            match self.code[j] {
                I::End => {
                    if j + 1 == self.code.len() {
                        return true;
                    }
                    j += 1;
                }
                I::Br(d) => {
                    // a `match` arm jumps to the end of its enclosing block with the arm's value
                    let mut left = d;
                    let mut nest = 0usize;
                    let mut k = j + 1;
                    loop {
                        match self.code.get(k) {
                            Some(I::Block(_) | I::Loop(_) | I::If(_)) => nest += 1,
                            Some(I::End) if nest > 0 => nest -= 1,
                            Some(I::End) if left == 0 => break,
                            Some(I::End) => left -= 1,
                            Some(_) => {}
                            None => return false,
                        }
                        k += 1;
                    }
                    j = k;
                }
                I::Else => {
                    // skip the else branch: it is followed by its own tail-or-not fate at the matching `end`
                    let mut depth = 0usize;
                    let mut k = j + 1;
                    loop {
                        match self.code.get(k) {
                            Some(I::Block(_) | I::Loop(_) | I::If(_)) => depth += 1,
                            Some(I::End) if depth == 0 => break,
                            Some(I::End) => depth -= 1,
                            Some(_) => {}
                            None => return false,
                        }
                        k += 1;
                    }
                    j = k;
                }
                _ => return false,
            }
        }
        false
    }

    fn finish(mut self) -> Function {
        let cands = std::mem::take(&mut self.tail_cands);
        for i in cands {
            if self.only_ends_after(i) {
                let swapped = match self.code[i] {
                    I::Call(f) => Some(I::ReturnCall(f)),
                    I::CallRef(t) => Some(I::ReturnCallRef(t)),
                    _ => None,
                };
                if let Some(t) = swapped {
                    self.code[i] = t;
                }
            }
        }
        let mut f = Function::new(self.extra.iter().map(|t| (1, *t)));
        for i in &self.code {
            f.instruction(i);
        }
        f
    }
}

struct Gen<'p> {
    prog: &'p Program,
    a: u32,
    c: u32,
    n: u32,
    sigs: Vec<Sig>,
    imports: Vec<(String, Sig)>,
    import_ix: HashMap<String, u32>,
    funcs: Vec<(u32, Option<Function>)>,
    fn_ix: HashMap<FnId, u32>,
    wrappers: HashMap<FnId, u32>,
    declared: BTreeSet<u32>,
    h: IntHelpers,
    strs: Vec<String>,
    str_g: HashMap<String, u32>,
    bigs: Vec<String>,
    big_g: HashMap<String, u32>,
    /// One shared object per nullary variant tag (`None`, `Red`, ...): no allocation at each use.
    nullary_g: HashMap<i32, u32>,
    n_globals: u32,
    const_g: Vec<u32>,
    cur: FnState,
    compiled: Vec<FnId>,
    option_ty: usize,
}

fn err<T>(m: impl Into<String>) -> Result<T, CompileError> {
    Err(CompileError(m.into()))
}

/// Compiles a checked program to a WasmGC module.
///
/// # Errors
/// Fails if the program uses a feature the code generator does not support.
pub fn compile(prog: &Program) -> Result<Module, CompileError> {
    let mut g = Gen::new(prog);
    g.run()
}

impl<'p> Gen<'p> {
    fn new(prog: &'p Program) -> Self {
        let option_ty = prog
            .types
            .iter()
            .position(|t| t.is_prelude && t.name == "Option")
            .unwrap_or(0);
        Self {
            prog,
            a: 4,
            c: 3,
            n: 4,
            sigs: Vec::new(),
            imports: Vec::new(),
            import_ix: HashMap::new(),
            funcs: Vec::new(),
            fn_ix: HashMap::new(),
            wrappers: HashMap::new(),
            declared: BTreeSet::new(),
            h: IntHelpers::default(),
            strs: Vec::new(),
            str_g: HashMap::new(),
            bigs: Vec::new(),
            big_g: HashMap::new(),
            nullary_g: HashMap::new(),
            n_globals: 0,
            const_g: Vec::new(),
            cur: FnState::new(0),
            compiled: Vec::new(),
            option_ty,
        }
    }

    // ------------------------------------------------------------ layout

    fn t_obj(&self, k: u32) -> u32 {
        k
    }
    fn t_f(&self) -> u32 {
        self.a + 1
    }
    fn t_env(&self) -> u32 {
        self.a + 2
    }
    fn t_fn(&self, k: u32) -> u32 {
        self.a + 3 + k
    }
    fn t_clo(&self, k: u32) -> u32 {
        self.a + 3 + (self.c + 1) + k
    }
    fn t_top(&self, n: u32) -> u32 {
        self.a + 3 + 2 * (self.c + 1) + n
    }
    fn t_sig(&self, sig: &Sig) -> u32 {
        let base = self.a + 3 + 2 * (self.c + 1) + (self.n + 1);
        base + self
            .sigs
            .iter()
            .position(|s| s == sig)
            .map_or(0, |i| u32::try_from(i).unwrap_or(0))
    }

    fn register_sig(&mut self, sig: &Sig) {
        if !self.sigs.contains(sig) {
            self.sigs.push(sig.clone());
        }
    }

    fn import(&mut self, name: &str, sig: Sig) -> u32 {
        if let Some(i) = self.import_ix.get(name) {
            return *i;
        }
        self.register_sig(&sig);
        let i = u32::try_from(self.imports.len()).unwrap_or(0);
        self.imports.push((name.to_string(), sig));
        self.import_ix.insert(name.to_string(), i);
        i
    }

    fn imp(&self, name: &str) -> u32 {
        self.import_ix.get(name).copied().unwrap_or(0)
    }

    // ------------------------------------------------------------ driver

    fn scan(&mut self) {
        use std::cell::Cell;
        let prog = self.prog;
        let (a, c, n) = (Cell::new(4u32), Cell::new(3u32), Cell::new(4u32));
        let len = |k: usize| u32::try_from(k).unwrap_or(0);
        let visit = |e: &Expr| match &e.kind {
            ExprKind::Ctor { args, .. } => a.set(a.get().max(len(args.len()))),
            ExprKind::Tuple(es) => a.set(a.get().max(len(es.len()))),
            ExprKind::Lambda { params, .. } => c.set(c.get().max(len(params.len()))),
            ExprKind::CallValue { args, .. } => c.set(c.get().max(len(args.len()))),
            ExprKind::With(_, ups) => a.set(a.get().max(len(ups.len()))),
            _ => {}
        };
        for f in &prog.fns {
            n.set(n.get().max(len(f.params.len())));
            c.set(c.get().max(len(f.params.len())));
            if let Some(b) = &f.body {
                b.walk(&mut { visit });
            }
        }
        for cst in &prog.consts {
            cst.value.walk(&mut { visit });
        }
        for t in &prog.types {
            match &t.kind {
                TypeKind::Record(fs) => a.set(a.get().max(len(fs.len()))),
                TypeKind::Adt(vs) => {
                    for v in vs {
                        a.set(a.get().max(len(v.fields.len())));
                    }
                }
            }
        }
        self.a = a.get();
        self.c = c.get();
        self.n = n.get();
    }

    fn run(&mut self) -> Result<Module, CompileError> {
        self.scan();
        for (name, sig) in fixed_imports() {
            self.import(name, sig);
        }
        for s in [Sig::F64Any, Sig::AnyF64, Sig::AnyToI32(1), Sig::AnyToI32(2)] {
            self.register_sig(&s);
        }
        let prog = self.prog;
        // functions to compile: everything with a body; externs become imports
        let mut order: Vec<FnId> = Vec::new();
        for (i, f) in prog.fns.iter().enumerate() {
            if f.is_extern {
                continue;
            }
            if f.body.is_some() {
                order.push(i);
            }
        }
        // collect native imports used anywhere
        let mut externs: BTreeSet<FnId> = BTreeSet::new();
        let mut note = |e: &Expr| match &e.kind {
            ExprKind::Call { f, .. } | ExprKind::FnRef(f, _) if prog.fns[*f].is_extern => {
                externs.insert(*f);
            }
            _ => {}
        };
        for &f in &order {
            if let Some(b) = &prog.fns[f].body {
                b.walk(&mut note);
            }
        }
        for cst in &prog.consts {
            cst.value.walk(&mut note);
        }
        for &f in &externs {
            let d = &prog.fns[f];
            let arity = u32::try_from(d.params.len()).unwrap_or(0);
            self.import(&d.full_name(), Sig::Any(arity));
        }
        let n_imports = u32::try_from(self.imports.len()).unwrap_or(0);

        // reserve function indices: helpers, then Tez functions
        let base = n_imports;
        self.h = IntHelpers {
            add: base,
            sub: base + 1,
            mul: base + 2,
            div: base + 3,
            rem: base + 4,
            neg: base + 5,
            lt: base + 6,
            le: base + 7,
            eq: base + 8,
        };
        let top2 = self.t_top(2);
        let top1 = self.t_top(1);
        let cmp = self.t_sig(&Sig::AnyToI32(2));
        for ty in [top2, top2, top2, top2, top2, top1, cmp, cmp, cmp] {
            self.funcs.push((ty, None));
        }
        for &f in &order {
            let ty = self.t_top(u32::try_from(prog.fns[f].params.len()).unwrap_or(0));
            let idx = n_imports + N_INT_HELPERS + u32::try_from(self.compiled.len()).unwrap_or(0);
            self.funcs.push((ty, None));
            self.fn_ix.insert(f, idx);
            self.compiled.push(f);
        }
        for _ in &prog.consts {
            self.const_g.push(self.n_globals);
            self.n_globals += 1;
        }

        self.build_int_helpers();
        for f in self.compiled.clone() {
            self.gen_fn(f)?;
        }
        let start = self.gen_start()?;
        let exports = self.gen_exports();
        Ok(Module {
            wasm: self.assemble(start, &exports),
        })
    }

    fn add_func(&mut self, ty: u32, body: Function) -> u32 {
        let idx = u32::try_from(self.imports.len() + self.funcs.len()).unwrap_or(0);
        self.funcs.push((ty, Some(body)));
        idx
    }

    fn set_body(&mut self, idx: u32, body: Function) {
        let n_imports = u32::try_from(self.imports.len()).unwrap_or(0);
        if let Some(slot) = self.funcs.get_mut((idx - n_imports) as usize) {
            slot.1 = Some(body);
        }
    }

    // ------------------------------------------------------------ small emit helpers

    fn ins(&mut self, i: I<'static>) {
        self.cur.code.push(i);
    }

    fn new_local(&mut self, t: ValType) -> u32 {
        self.cur.extra.push(t);
        self.cur.nparams + u32::try_from(self.cur.extra.len() - 1).unwrap_or(0)
    }

    fn slot_local(&mut self, slot: u32) -> u32 {
        if let Some(l) = self.cur.slots.get(&slot) {
            return *l;
        }
        let l = self.new_local(ANY);
        self.cur.slots.insert(slot, l);
        l
    }

    fn open(&mut self, i: I<'static>) -> u32 {
        self.ins(i);
        self.cur.depth += 1;
        self.cur.depth
    }

    fn close(&mut self) {
        self.ins(I::End);
        self.cur.depth -= 1;
    }

    fn br(&mut self, label: u32) {
        let d = self.cur.depth - label;
        self.ins(I::Br(d));
    }

    fn br_if(&mut self, label: u32) {
        let d = self.cur.depth - label;
        self.ins(I::BrIf(d));
    }

    fn box_bool(&mut self) {
        self.ins(I::RefI31);
    }

    fn unbox_i32(&mut self) {
        self.ins(I::RefCastNonNull(I31));
        self.ins(I::I31GetU);
    }

    fn unbox_f64(&mut self) {
        let t = self.t_f();
        self.ins(I::RefCastNonNull(concrete(t)));
        self.ins(I::StructGet {
            struct_type_index: t,
            field_index: 0,
        });
    }

    fn box_f64(&mut self) {
        let t = self.t_f();
        self.ins(I::StructNew(t));
    }

    // ------------------------------------------------------------ literals

    fn str_global(&mut self, s: &str) -> u32 {
        if let Some(g) = self.str_g.get(s) {
            return *g;
        }
        let g = self.n_globals;
        self.n_globals += 1;
        self.strs.push(s.to_string());
        self.str_g.insert(s.to_string(), g);
        g
    }

    fn big_global(&mut self, s: &str) -> u32 {
        if let Some(g) = self.big_g.get(s) {
            return *g;
        }
        let g = self.n_globals;
        self.n_globals += 1;
        self.bigs.push(s.to_string());
        self.big_g.insert(s.to_string(), g);
        g
    }

    fn push_int(&mut self, n: &BigInt) {
        match n.to_i64() {
            Some(v) if (FIT_MIN..FIT_MAX).contains(&v) => {
                self.ins(I::I32Const(i32::try_from(v).unwrap_or(0)));
                self.ins(I::RefI31);
            }
            Some(v) => {
                self.ins(I::I64Const(v));
                let f = self.imp("big_from_i64");
                self.ins(I::Call(f));
            }
            None => {
                let g = self.big_global(&n.to_string());
                self.ins(I::GlobalGet(g));
            }
        }
    }

    // ------------------------------------------------------------ integer helpers

    fn i31_to_i64(code: &mut Vec<I<'static>>, local: u32) {
        code.push(I::LocalGet(local));
        code.push(I::RefCastNonNull(I31));
        code.push(I::I31GetS);
        code.push(I::I64ExtendI32S);
    }

    fn arith_helper(&self, op: I<'static>, slow: u32, from_i64: u32) -> Function {
        let mut c: Vec<I<'static>> = Vec::new();
        self.both_i31_static(&mut c);
        c.push(I::If(BlockType::Result(ANY)));
        Self::i31_to_i64(&mut c, 0);
        Self::i31_to_i64(&mut c, 1);
        c.push(op);
        c.push(I::LocalSet(2));
        c.extend(Self::fit_check_and_box(2, from_i64));
        c.push(I::Else);
        c.push(I::LocalGet(0));
        c.push(I::LocalGet(1));
        c.push(I::Call(slow));
        c.push(I::End);
        c.push(I::End);
        let mut f = Function::new([(1, ValType::I64)]);
        for i in &c {
            f.instruction(i);
        }
        f
    }

    fn both_i31_static(&self, code: &mut Vec<I<'static>>) {
        code.push(I::LocalGet(0));
        code.push(I::RefTestNonNull(I31));
        code.push(I::LocalGet(1));
        code.push(I::RefTestNonNull(I31));
        code.push(I::I32And);
    }

    /// `if r fits in 31 bits { ref.i31 r } else { big_from_i64(r) }` for the i64 local `r`.
    fn fit_check_and_box(r: u32, from_i64: u32) -> Vec<I<'static>> {
        vec![
            I::LocalGet(r),
            I::I64Const(FIT_MIN),
            I::I64GeS,
            I::LocalGet(r),
            I::I64Const(FIT_MAX),
            I::I64LtS,
            I::I32And,
            I::If(BlockType::Result(ANY)),
            I::LocalGet(r),
            I::I32WrapI64,
            I::RefI31,
            I::Else,
            I::LocalGet(r),
            I::Call(from_i64),
            I::End,
        ]
    }

    fn build_int_helpers(&mut self) {
        let from_i64 = self.imp("big_from_i64");
        let ops: [(u32, I<'static>, &str); 5] = [
            (self.h.add, I::I64Add, "big_add"),
            (self.h.sub, I::I64Sub, "big_sub"),
            (self.h.mul, I::I64Mul, "big_mul"),
            (self.h.div, I::I64DivS, "big_div"),
            (self.h.rem, I::I64RemS, "big_rem"),
        ];
        for (idx, op, slow) in ops {
            let f = self.arith_helper(op, self.imp(slow), from_i64);
            self.set_body(idx, f);
        }
        // negation
        let mut c: Vec<I<'static>> = vec![
            I::LocalGet(0),
            I::RefTestNonNull(I31),
            I::If(BlockType::Result(ANY)),
            I::I64Const(0),
        ];
        Self::i31_to_i64(&mut c, 0);
        c.push(I::I64Sub);
        c.push(I::LocalSet(1));
        c.extend(Self::fit_check_and_box(1, from_i64));
        c.push(I::Else);
        c.push(I::LocalGet(0));
        c.push(I::Call(self.imp("big_neg")));
        c.push(I::End);
        c.push(I::End);
        let mut f = Function::new([(1, ValType::I64)]);
        for i in &c {
            f.instruction(i);
        }
        self.set_body(self.h.neg, f);
        // comparisons
        for (idx, op, slow) in [
            (self.h.lt, I::I32LtS, "big_lt"),
            (self.h.le, I::I32LeS, "big_le"),
            (self.h.eq, I::I32Eq, "big_eq"),
        ] {
            let mut c: Vec<I<'static>> = Vec::new();
            self.both_i31_static(&mut c);
            c.push(I::If(BlockType::Result(ValType::I32)));
            for l in [0, 1] {
                c.push(I::LocalGet(l));
                c.push(I::RefCastNonNull(I31));
                c.push(I::I31GetS);
            }
            c.push(op);
            c.push(I::Else);
            c.push(I::LocalGet(0));
            c.push(I::LocalGet(1));
            c.push(I::Call(self.imp(slow)));
            c.push(I::End);
            c.push(I::End);
            let mut f = Function::new([]);
            for i in &c {
                f.instruction(i);
            }
            self.set_body(idx, f);
        }
    }

    // ------------------------------------------------------------ functions

    fn gen_fn(&mut self, f: FnId) -> Result<(), CompileError> {
        let prog = self.prog;
        let def = &prog.fns[f];
        let Some(body) = &def.body else { return Ok(()) };
        let saved = std::mem::replace(
            &mut self.cur,
            FnState::new(u32::try_from(def.params.len()).unwrap_or(0)),
        );
        self.cur.tail_ok = true;
        for (i, p) in def.params.iter().enumerate() {
            self.cur
                .slots
                .insert(p.local, u32::try_from(i).unwrap_or(0));
        }
        self.expr(body)?;
        self.ins(I::End);
        let st = std::mem::replace(&mut self.cur, saved);
        let idx = self.fn_ix[&f];
        self.set_body(idx, st.finish());
        Ok(())
    }

    // ------------------------------------------------------------ expressions

    fn expr(&mut self, e: &'p Expr) -> Result<(), CompileError> {
        match &e.kind {
            ExprKind::Int(n) => self.push_int(n),
            ExprKind::Float(f) => {
                self.ins(I::F64Const((*f).into()));
                self.box_f64();
            }
            ExprKind::Str(s) => {
                let g = self.str_global(s);
                self.ins(I::GlobalGet(g));
            }
            ExprKind::Bool(b) => {
                self.ins(I::I32Const(i32::from(*b)));
                self.ins(I::RefI31);
            }
            ExprKind::Unit => {
                self.ins(I::I32Const(0));
                self.ins(I::RefI31);
            }
            ExprKind::Local(i) => {
                let l = self.slot_local(*i);
                self.ins(I::LocalGet(l));
            }
            ExprKind::Const(c) => {
                let g = self.const_g[*c];
                self.ins(I::GlobalGet(g));
            }
            ExprKind::FnRef(f, _) => self.fn_value(*f)?,
            ExprKind::Call { f, args, .. } => {
                for a in args {
                    self.expr(a)?;
                }
                let d = &self.prog.fns[*f];
                let idx = if d.is_extern {
                    self.imp(&d.full_name())
                } else {
                    self.fn_ix[f]
                };
                if self.cur.tail_ok && !d.is_extern {
                    self.cur.tail_cands.push(self.cur.code.len());
                }
                self.ins(I::Call(idx));
            }
            ExprKind::CallValue { callee, args } => self.call_value(callee, args)?,
            ExprKind::Ctor {
                variant, args, ty, ..
            } => {
                let tag = match &self.prog.types[*ty].kind {
                    TypeKind::Record(_) => 0,
                    TypeKind::Adt(_) => i32::try_from(*variant).unwrap_or(0),
                };
                self.object(tag, args)?;
            }
            ExprKind::Field(b, i) | ExprKind::TupleField(b, i) => {
                self.expr(b)?;
                self.field_get(u32::try_from(*i).unwrap_or(0));
            }
            ExprKind::With(b, ups) => self.with(b, ups)?,
            ExprKind::Tuple(es) => self.object(0, es)?,
            ExprKind::List(es) => {
                let new = self.imp("list_new");
                let push = self.imp("list_push");
                self.ins(I::Call(new));
                for x in es {
                    self.expr(x)?;
                    self.ins(I::Call(push));
                }
            }
            ExprKind::Index(b, i) => {
                self.expr(b)?;
                self.expr(i)?;
                self.ins(I::RefCastNonNull(I31));
                self.ins(I::I31GetS);
                let g = self.imp("list_get");
                self.ins(I::Call(g));
            }
            ExprKind::Unary(op, x) => match (op, &x.ty) {
                (UnOp::Not, _) => {
                    self.cond(e)?;
                    self.box_bool();
                }
                (UnOp::Neg, Ty::Float) => {
                    self.f64_expr(x)?;
                    self.ins(I::F64Neg);
                    self.box_f64();
                }
                (UnOp::Neg, _) => {
                    self.expr(x)?;
                    let h = self.h.neg;
                    self.ins(I::Call(h));
                }
            },
            ExprKind::Binary(op, l, r) => self.binary(e, *op, l, r)?,
            ExprKind::If(c, t, f) => {
                self.cond(c)?;
                self.open(I::If(BlockType::Result(ANY)));
                self.expr(t)?;
                self.ins(I::Else);
                self.expr(f)?;
                self.close();
            }
            ExprKind::Match(s, arms) => self.match_expr(s, arms)?,
            ExprKind::Block(stmts, tail) => {
                for st in stmts {
                    match st {
                        Stmt::Let { pat, init, .. } => {
                            self.expr(init)?;
                            let tmp = self.new_local(ANY);
                            self.ins(I::LocalSet(tmp));
                            self.pattern(pat, tmp, None)?;
                        }
                        Stmt::Assert(c) => {
                            self.cond(c)?;
                            self.ins(I::I32Eqz);
                            self.open(I::If(BlockType::Empty));
                            self.ins(I::I32Const(0));
                            let fail = self.imp("fail");
                            self.ins(I::Call(fail));
                            self.close();
                        }
                    }
                }
                self.expr(tail)?;
            }
            ExprKind::Lambda { .. } => self.lambda(e)?,
        }
        Ok(())
    }

    fn f64_expr(&mut self, e: &'p Expr) -> Result<(), CompileError> {
        self.expr(e)?;
        self.unbox_f64();
        Ok(())
    }

    fn field_get(&mut self, i: u32) {
        let t = self.t_obj(i + 1);
        self.ins(I::RefCastNonNull(concrete(t)));
        self.ins(I::StructGet {
            struct_type_index: t,
            field_index: 2 + i,
        });
    }

    fn object(&mut self, tag: i32, fields: &'p [Expr]) -> Result<(), CompileError> {
        if fields.is_empty() {
            let g = match self.nullary_g.get(&tag) {
                Some(g) => *g,
                None => {
                    let g = self.n_globals;
                    self.n_globals += 1;
                    self.nullary_g.insert(tag, g);
                    g
                }
            };
            self.ins(I::GlobalGet(g));
            return Ok(());
        }
        let n = u32::try_from(fields.len()).unwrap_or(0);
        self.ins(I::I32Const(tag));
        self.ins(I::I32Const(i32::try_from(n).unwrap_or(0)));
        for f in fields {
            self.expr(f)?;
        }
        let t = self.t_obj(n);
        self.ins(I::StructNew(t));
        Ok(())
    }

    fn with(&mut self, b: &'p Expr, ups: &'p [(usize, Expr)]) -> Result<(), CompileError> {
        let Ty::Adt(tid, _) = &b.ty else {
            return err("`with` on a non-record");
        };
        let n = match &self.prog.types[*tid].kind {
            TypeKind::Record(fs) => u32::try_from(fs.len()).unwrap_or(0),
            TypeKind::Adt(_) => return err("`with` on a sum type"),
        };
        self.expr(b)?;
        let tmp = self.new_local(ANY);
        self.ins(I::LocalSet(tmp));
        self.ins(I::I32Const(0));
        self.ins(I::I32Const(i32::try_from(n).unwrap_or(0)));
        for j in 0..n {
            if let Some((_, ue)) = ups.iter().find(|(k, _)| *k == j as usize) {
                self.expr(ue)?;
            } else {
                self.ins(I::LocalGet(tmp));
                self.field_get(j);
            }
        }
        let t = self.t_obj(n);
        self.ins(I::StructNew(t));
        Ok(())
    }

    // ------------------------------------------------------------ conditions and operators

    /// Pushes an `i32` truth value.
    fn cond(&mut self, e: &'p Expr) -> Result<(), CompileError> {
        match &e.kind {
            ExprKind::Bool(b) => self.ins(I::I32Const(i32::from(*b))),
            ExprKind::Unary(UnOp::Not, x) => {
                self.cond(x)?;
                self.ins(I::I32Eqz);
            }
            ExprKind::Binary(op, l, r) if self.binary_cond(*op, l, r)? => {}
            _ => {
                self.expr(e)?;
                self.unbox_i32();
            }
        }
        Ok(())
    }

    /// Emits comparison / logic operators as `i32`; false when `op` is arithmetic.
    fn binary_cond(&mut self, op: BinOp, l: &'p Expr, r: &'p Expr) -> Result<bool, CompileError> {
        match op {
            BinOp::And => {
                self.cond(l)?;
                self.open(I::If(BlockType::Result(ValType::I32)));
                self.cond(r)?;
                self.ins(I::Else);
                self.ins(I::I32Const(0));
                self.close();
            }
            BinOp::Or => {
                self.cond(l)?;
                self.open(I::If(BlockType::Result(ValType::I32)));
                self.ins(I::I32Const(1));
                self.ins(I::Else);
                self.cond(r)?;
                self.close();
            }
            BinOp::Eq | BinOp::Ne => {
                self.equality(&l.ty, l, r)?;
                if op == BinOp::Ne {
                    self.ins(I::I32Eqz);
                }
            }
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => self.ordering(op, l, r)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn equality(&mut self, ty: &Ty, l: &'p Expr, r: &'p Expr) -> Result<(), CompileError> {
        match ty {
            Ty::Int => {
                self.expr(l)?;
                self.expr(r)?;
                let h = self.h.eq;
                self.ins(I::Call(h));
            }
            Ty::Bool | Ty::Unit => {
                self.expr(l)?;
                self.unbox_i32();
                self.expr(r)?;
                self.unbox_i32();
                self.ins(I::I32Eq);
            }
            Ty::Float => {
                self.f64_expr(l)?;
                self.f64_expr(r)?;
                self.ins(I::F64Eq);
            }
            Ty::Str => {
                self.expr(l)?;
                self.expr(r)?;
                let f = self.imp("str_eq");
                self.ins(I::Call(f));
            }
            _ => {
                self.expr(l)?;
                self.expr(r)?;
                let f = self.imp("deep_eq");
                self.ins(I::Call(f));
            }
        }
        Ok(())
    }

    fn ordering(&mut self, op: BinOp, l: &'p Expr, r: &'p Expr) -> Result<(), CompileError> {
        match &l.ty {
            Ty::Float => {
                self.f64_expr(l)?;
                self.f64_expr(r)?;
                self.ins(match op {
                    BinOp::Lt => I::F64Lt,
                    BinOp::Le => I::F64Le,
                    BinOp::Gt => I::F64Gt,
                    _ => I::F64Ge,
                });
            }
            ty => {
                let (lt, le) = if matches!(ty, Ty::Str) {
                    (self.imp("str_lt"), self.imp("str_le"))
                } else {
                    (self.h.lt, self.h.le)
                };
                let (first, second, f) = match op {
                    BinOp::Lt => (l, r, lt),
                    BinOp::Le => (l, r, le),
                    BinOp::Gt => (r, l, lt),
                    _ => (r, l, le),
                };
                self.expr(first)?;
                self.expr(second)?;
                self.ins(I::Call(f));
            }
        }
        Ok(())
    }

    fn binary(
        &mut self,
        e: &'p Expr,
        op: BinOp,
        l: &'p Expr,
        r: &'p Expr,
    ) -> Result<(), CompileError> {
        if matches!(
            op,
            BinOp::And
                | BinOp::Or
                | BinOp::Eq
                | BinOp::Ne
                | BinOp::Lt
                | BinOp::Le
                | BinOp::Gt
                | BinOp::Ge
        ) {
            self.cond(e)?;
            self.box_bool();
            return Ok(());
        }
        if op == BinOp::Concat {
            self.expr(l)?;
            self.expr(r)?;
            let f = self.imp("str_concat");
            self.ins(I::Call(f));
            return Ok(());
        }
        if l.ty == Ty::Float {
            self.f64_expr(l)?;
            self.f64_expr(r)?;
            self.ins(match op {
                BinOp::Add => I::F64Add,
                BinOp::Sub => I::F64Sub,
                BinOp::Mul => I::F64Mul,
                _ => I::F64Div,
            });
            self.box_f64();
            return Ok(());
        }
        self.expr(l)?;
        self.expr(r)?;
        let h = match op {
            BinOp::Add => self.h.add,
            BinOp::Sub => self.h.sub,
            BinOp::Mul => self.h.mul,
            BinOp::Div => self.h.div,
            _ => self.h.rem,
        };
        self.ins(I::Call(h));
        Ok(())
    }

    // ------------------------------------------------------------ patterns and match

    /// Tests `p` against the value in local `subj`, binding variables. When `fail` is `Some`,
    /// a mismatch branches to that label; with `None` the pattern is irrefutable (no tests).
    fn pattern(&mut self, p: &'p Pat, subj: u32, fail: Option<u32>) -> Result<(), CompileError> {
        match &p.kind {
            PatKind::Wild | PatKind::Unit => {}
            PatKind::Bind(slot) => {
                let l = self.slot_local(*slot);
                self.ins(I::LocalGet(subj));
                self.ins(I::LocalSet(l));
            }
            PatKind::Lit(lit) => {
                let Some(label) = fail else { return Ok(()) };
                self.literal_mismatch(lit, subj);
                self.br_if(label);
            }
            PatKind::Ctor { ty, variant, args } => {
                let multi = self.prog.types[*ty].variant_count() > 1;
                if let (Some(label), true) = (fail, multi) {
                    self.ins(I::LocalGet(subj));
                    let t0 = self.t_obj(0);
                    self.ins(I::RefCastNonNull(concrete(t0)));
                    self.ins(I::StructGet {
                        struct_type_index: t0,
                        field_index: 0,
                    });
                    self.ins(I::I32Const(i32::try_from(*variant).unwrap_or(0)));
                    self.ins(I::I32Ne);
                    self.br_if(label);
                }
                self.sub_patterns(args, subj, fail)?;
            }
            PatKind::Tuple(ps) => self.sub_patterns(ps, subj, fail)?,
            PatKind::List { items, rest } => {
                if let Some(label) = fail {
                    self.ins(I::LocalGet(subj));
                    let len = self.imp("list_len");
                    self.ins(I::Call(len));
                    self.ins(I::I32Const(i32::try_from(items.len()).unwrap_or(0)));
                    self.ins(if rest.is_some() { I::I32LtS } else { I::I32Ne });
                    self.br_if(label);
                }
                for (j, it) in items.iter().enumerate() {
                    if matches!(it.kind, PatKind::Wild) {
                        continue;
                    }
                    let tmp = self.new_local(ANY);
                    self.ins(I::LocalGet(subj));
                    self.ins(I::I32Const(i32::try_from(j).unwrap_or(0)));
                    let get = self.imp("list_get");
                    self.ins(I::Call(get));
                    self.ins(I::LocalSet(tmp));
                    self.pattern(it, tmp, fail)?;
                }
                if let Some(Some(slot)) = rest {
                    let l = self.slot_local(*slot);
                    self.ins(I::LocalGet(subj));
                    self.ins(I::I32Const(i32::try_from(items.len()).unwrap_or(0)));
                    let slice = self.imp("list_slice");
                    self.ins(I::Call(slice));
                    self.ins(I::LocalSet(l));
                }
            }
        }
        Ok(())
    }

    fn sub_patterns(
        &mut self,
        ps: &'p [Pat],
        subj: u32,
        fail: Option<u32>,
    ) -> Result<(), CompileError> {
        let n = u32::try_from(ps.len()).unwrap_or(0);
        for (j, sp) in ps.iter().enumerate() {
            if matches!(sp.kind, PatKind::Wild | PatKind::Unit) {
                continue;
            }
            let tmp = self.new_local(ANY);
            self.ins(I::LocalGet(subj));
            let t = self.t_obj(n);
            self.ins(I::RefCastNonNull(concrete(t)));
            self.ins(I::StructGet {
                struct_type_index: t,
                field_index: 2 + u32::try_from(j).unwrap_or(0),
            });
            self.ins(I::LocalSet(tmp));
            self.pattern(sp, tmp, fail)?;
        }
        Ok(())
    }

    /// Pushes `i32` 1 when the literal pattern does NOT match the value in `subj`.
    fn literal_mismatch(&mut self, lit: &LitVal, subj: u32) {
        match lit {
            LitVal::Int(n) => {
                self.ins(I::LocalGet(subj));
                self.push_int(n);
                let h = self.h.eq;
                self.ins(I::Call(h));
                self.ins(I::I32Eqz);
            }
            LitVal::Bool(b) => {
                self.ins(I::LocalGet(subj));
                self.unbox_i32();
                self.ins(I::I32Const(i32::from(*b)));
                self.ins(I::I32Ne);
            }
            LitVal::Float(f) => {
                self.ins(I::LocalGet(subj));
                self.unbox_f64();
                self.ins(I::F64Const((*f).into()));
                self.ins(I::F64Ne);
            }
            LitVal::Str(s) => {
                let g = self.str_global(s);
                self.ins(I::LocalGet(subj));
                self.ins(I::GlobalGet(g));
                let eq = self.imp("str_eq");
                self.ins(I::Call(eq));
                self.ins(I::I32Eqz);
            }
        }
    }

    fn match_expr(
        &mut self,
        s: &'p Expr,
        arms: &'p [tez_types::hir::Arm],
    ) -> Result<(), CompileError> {
        self.expr(s)?;
        let subj = self.new_local(ANY);
        self.ins(I::LocalSet(subj));
        let done = self.open(I::Block(BlockType::Result(ANY)));
        for arm in arms {
            let fail = self.open(I::Block(BlockType::Empty));
            self.pattern(&arm.pat, subj, Some(fail))?;
            if let Some(g) = &arm.guard {
                self.cond(g)?;
                self.ins(I::I32Eqz);
                self.br_if(fail);
            }
            self.expr(&arm.body)?;
            self.br(done);
            self.close();
        }
        self.ins(I::Unreachable);
        self.close();
        Ok(())
    }

    // ------------------------------------------------------------ functions as values

    fn closure_call_parts(&mut self, k: u32) -> (u32, u32, u32) {
        (self.t_clo(k), self.t_fn(k), self.t_env())
    }

    fn call_value(&mut self, callee: &'p Expr, args: &'p [Expr]) -> Result<(), CompileError> {
        let k = u32::try_from(args.len()).unwrap_or(0);
        let (tclo, tfn, _) = self.closure_call_parts(k);
        self.expr(callee)?;
        let tmp = self.new_local(ANY);
        self.ins(I::LocalSet(tmp));
        // env first
        self.ins(I::LocalGet(tmp));
        self.ins(I::RefCastNonNull(concrete(tclo)));
        self.ins(I::StructGet {
            struct_type_index: tclo,
            field_index: 1,
        });
        for a in args {
            self.expr(a)?;
        }
        self.ins(I::LocalGet(tmp));
        self.ins(I::RefCastNonNull(concrete(tclo)));
        self.ins(I::StructGet {
            struct_type_index: tclo,
            field_index: 0,
        });
        if self.cur.tail_ok {
            self.cur.tail_cands.push(self.cur.code.len());
        }
        self.ins(I::CallRef(tfn));
        Ok(())
    }

    /// A named function as a value: a closure over a wrapper that ignores its environment.
    fn fn_value(&mut self, f: FnId) -> Result<(), CompileError> {
        let prog = self.prog;
        let d = &prog.fns[f];
        let k = u32::try_from(d.params.len()).unwrap_or(0);
        let wrapper = match self.wrappers.get(&f) {
            Some(w) => *w,
            None => {
                let mut st = FnState::new(1 + k);
                for i in 0..k {
                    st.code.push(I::LocalGet(1 + i));
                }
                let target = if d.is_extern {
                    self.imp(&d.full_name())
                } else {
                    self.fn_ix[&f]
                };
                st.code.push(I::Call(target));
                st.code.push(I::End);
                let ty = self.t_fn(k);
                let idx = self.add_func(ty, st.finish());
                self.declared.insert(idx);
                self.wrappers.insert(f, idx);
                idx
            }
        };
        let (tclo, _, tenv) = self.closure_call_parts(k);
        self.ins(I::RefFunc(wrapper));
        self.ins(I::RefNull(concrete(tenv)));
        self.ins(I::StructNew(tclo));
        Ok(())
    }

    fn free_vars(e: &Expr) -> Vec<u32> {
        let mut used: BTreeSet<u32> = BTreeSet::new();
        let mut bound: BTreeSet<u32> = BTreeSet::new();
        fn bind(p: &Pat, bound: &mut BTreeSet<u32>) {
            match &p.kind {
                PatKind::Bind(s) => {
                    bound.insert(*s);
                }
                PatKind::Ctor { args, .. } => args.iter().for_each(|a| bind(a, bound)),
                PatKind::Tuple(ps) => ps.iter().for_each(|a| bind(a, bound)),
                PatKind::List { items, rest } => {
                    items.iter().for_each(|a| bind(a, bound));
                    if let Some(Some(s)) = rest {
                        bound.insert(*s);
                    }
                }
                PatKind::Wild | PatKind::Unit | PatKind::Lit(_) => {}
            }
        }
        e.walk(&mut |x| match &x.kind {
            ExprKind::Local(i) => {
                used.insert(*i);
            }
            ExprKind::Lambda { params, .. } => bound.extend(params.iter().copied()),
            ExprKind::Match(_, arms) => arms.iter().for_each(|a| bind(&a.pat, &mut bound)),
            ExprKind::Block(stmts, _) => {
                for s in stmts {
                    if let Stmt::Let { pat, .. } = s {
                        bind(pat, &mut bound);
                    }
                }
            }
            _ => {}
        });
        used.difference(&bound).copied().collect()
    }

    fn lambda(&mut self, e: &'p Expr) -> Result<(), CompileError> {
        let ExprKind::Lambda { params, body } = &e.kind else {
            return err("not a lambda");
        };
        let k = u32::try_from(params.len()).unwrap_or(0);
        let free = Self::free_vars(e);
        // the lambda's own function
        let saved = std::mem::replace(&mut self.cur, FnState::new(1 + k));
        self.cur.tail_ok = true;
        for (i, slot) in params.iter().enumerate() {
            self.cur
                .slots
                .insert(*slot, 1 + u32::try_from(i).unwrap_or(0));
        }
        let tenv = self.t_env();
        for (j, slot) in free.iter().enumerate() {
            let l = self.new_local(ANY);
            self.cur.slots.insert(*slot, l);
            self.ins(I::LocalGet(0));
            self.ins(I::I32Const(i32::try_from(j).unwrap_or(0)));
            self.ins(I::ArrayGet(tenv));
            self.ins(I::LocalSet(l));
        }
        self.expr(body)?;
        self.ins(I::End);
        let st = std::mem::replace(&mut self.cur, saved);
        let ty = self.t_fn(k);
        let idx = self.add_func(ty, st.finish());
        self.declared.insert(idx);
        // the closure value
        let tclo = self.t_clo(k);
        self.ins(I::RefFunc(idx));
        if free.is_empty() {
            self.ins(I::RefNull(concrete(tenv)));
        } else {
            for slot in &free {
                let l = self.slot_local(*slot);
                self.ins(I::LocalGet(l));
            }
            self.ins(I::ArrayNewFixed {
                array_type_index: tenv,
                array_size: u32::try_from(free.len()).unwrap_or(0),
            });
        }
        self.ins(I::StructNew(tclo));
        Ok(())
    }

    // ------------------------------------------------------------ start function and exports

    fn gen_start(&mut self) -> Result<u32, CompileError> {
        let prog = self.prog;
        let saved = std::mem::replace(&mut self.cur, FnState::new(0));
        // constants first (literal globals are filled below, before any code runs)
        for (i, c) in prog.consts.iter().enumerate() {
            self.cur.slots.clear();
            self.expr(&c.value)?;
            let g = self.const_g[i];
            self.ins(I::GlobalSet(g));
        }
        let mut body = std::mem::take(&mut self.cur.code);
        // literal initialisation goes first
        let mut init: Vec<I<'static>> = Vec::new();
        let str_lit = self.imp("str_lit");
        let big_lit = self.imp("big_lit");
        for (i, s) in self.strs.iter().enumerate() {
            let g = self.str_g[s];
            init.push(I::I32Const(i32::try_from(i).unwrap_or(0)));
            init.push(I::Call(str_lit));
            init.push(I::GlobalSet(g));
        }
        for (i, s) in self.bigs.iter().enumerate() {
            let g = self.big_g[s];
            init.push(I::I32Const(i32::try_from(i).unwrap_or(0)));
            init.push(I::Call(big_lit));
            init.push(I::GlobalSet(g));
        }
        let t0 = self.t_obj(0);
        let mut nullary: Vec<(i32, u32)> = self.nullary_g.iter().map(|(t, g)| (*t, *g)).collect();
        nullary.sort_unstable();
        for (tag, g) in nullary {
            init.push(I::I32Const(tag));
            init.push(I::I32Const(0));
            init.push(I::StructNew(t0));
            init.push(I::GlobalSet(g));
        }
        init.append(&mut body);
        self.cur.code = init;
        self.ins(I::End);
        let st = std::mem::replace(&mut self.cur, saved);
        let ty = self.t_start();
        Ok(self.add_func(ty, st.finish()))
    }

    /// Type index of `() -> ()` (appended after all signatures).
    fn t_start(&self) -> u32 {
        self.a + 3 + 2 * (self.c + 1) + (self.n + 1) + u32::try_from(self.sigs.len()).unwrap_or(0)
    }

    fn gen_exports(&mut self) -> Vec<(String, u32)> {
        let prog = self.prog;
        let mut out: Vec<(String, u32)> = Vec::new();
        for (f, d) in prog.fns.iter().enumerate() {
            if !d.is_prelude && d.body.is_some() {
                out.push((format!("f_{}", d.name), self.fn_ix[&f]));
            }
        }
        // helpers the JS runtime needs to read and build values
        let t_f = self.t_f();
        let t0 = self.t_obj(0);
        let ft = self.t_sig(&Sig::F64Any);
        let fget = self.t_sig(&Sig::AnyF64);
        let to_i32 = self.t_sig(&Sig::AnyToI32(1));
        let top1 = self.t_top(1);

        let mut f = FnState::new(1);
        f.code.extend([I::LocalGet(0), I::StructNew(t_f), I::End]);
        let i = self.add_func(ft, f.finish());
        out.push(("__f_new".into(), i));

        let mut f = FnState::new(1);
        f.code.extend([
            I::LocalGet(0),
            I::RefCastNonNull(concrete(t_f)),
            I::StructGet {
                struct_type_index: t_f,
                field_index: 0,
            },
            I::End,
        ]);
        let i = self.add_func(fget, f.finish());
        out.push(("__f_get".into(), i));

        let mut f = FnState::new(1);
        f.code
            .extend([I::LocalGet(0), I::RefTestNonNull(concrete(t_f)), I::End]);
        let i = self.add_func(to_i32, f.finish());
        out.push(("__is_f".into(), i));

        let mut f = FnState::new(1);
        f.code
            .extend([I::LocalGet(0), I::RefTestNonNull(concrete(t0)), I::End]);
        let i = self.add_func(to_i32, f.finish());
        out.push(("__is_obj".into(), i));

        for (name, field) in [("__tag", 0u32), ("__len", 1u32)] {
            let mut f = FnState::new(1);
            f.code.extend([
                I::LocalGet(0),
                I::RefCastNonNull(concrete(t0)),
                I::StructGet {
                    struct_type_index: t0,
                    field_index: field,
                },
                I::End,
            ]);
            let i = self.add_func(to_i32, f.finish());
            out.push((name.into(), i));
        }
        for j in 0..self.a {
            let t = self.t_obj(j + 1);
            let mut f = FnState::new(1);
            f.code.extend([
                I::LocalGet(0),
                I::RefCastNonNull(concrete(t)),
                I::StructGet {
                    struct_type_index: t,
                    field_index: 2 + j,
                },
                I::End,
            ]);
            let i = self.add_func(top1, f.finish());
            out.push((format!("__get{j}"), i));
        }
        // Option constructors for natives
        let t1 = self.t_obj(1);
        let mut f = FnState::new(1);
        f.code.extend([
            I::I32Const(1),
            I::I32Const(1),
            I::LocalGet(0),
            I::StructNew(t1),
            I::End,
        ]);
        let i = self.add_func(top1, f.finish());
        out.push(("__some".into(), i));
        let top0 = self.t_top(0);
        let mut f = FnState::new(0);
        f.code
            .extend([I::I32Const(0), I::I32Const(0), I::StructNew(t0), I::End]);
        let i = self.add_func(top0, f.finish());
        out.push(("__none".into(), i));
        // tuples
        for k in 2..=3u32 {
            let t = self.t_obj(k);
            let mut f = FnState::new(k);
            f.code
                .extend([I::I32Const(0), I::I32Const(i32::try_from(k).unwrap_or(0))]);
            for j in 0..k {
                f.code.push(I::LocalGet(j));
            }
            f.code.extend([I::StructNew(t), I::End]);
            let ty = self.t_top(k);
            let i = self.add_func(ty, f.finish());
            out.push((format!("__tup{k}"), i));
        }
        // closure application for natives
        for k in 1..=3u32 {
            let (tclo, tfn, _) = self.closure_call_parts(k);
            let mut f = FnState::new(1 + k);
            f.code.extend([
                I::LocalGet(0),
                I::RefCastNonNull(concrete(tclo)),
                I::StructGet {
                    struct_type_index: tclo,
                    field_index: 1,
                },
            ]);
            for j in 1..=k {
                f.code.push(I::LocalGet(j));
            }
            f.code.extend([
                I::LocalGet(0),
                I::RefCastNonNull(concrete(tclo)),
                I::StructGet {
                    struct_type_index: tclo,
                    field_index: 0,
                },
                I::CallRef(tfn),
                I::End,
            ]);
            let ty = self.t_top(1 + k);
            let i = self.add_func(ty, f.finish());
            out.push((format!("__apply{k}"), i));
        }
        out
    }

    // ------------------------------------------------------------ module assembly

    fn ty_json(&self, t: &Ty) -> Json {
        match t {
            Ty::Int => json!({"k": "int"}),
            Ty::Float => json!({"k": "float"}),
            Ty::Bool => json!({"k": "bool"}),
            Ty::Str => json!({"k": "str"}),
            Ty::Unit => json!({"k": "unit"}),
            Ty::List(e) => json!({"k": "list", "t": self.ty_json(e)}),
            Ty::Tuple(ts) => {
                json!({"k": "tuple", "ts": ts.iter().map(|x| self.ty_json(x)).collect::<Vec<_>>()})
            }
            Ty::Fn(..) => json!({"k": "fn"}),
            Ty::Adt(id, args) => {
                json!({"k": "adt", "id": id, "args": args.iter().map(|x| self.ty_json(x)).collect::<Vec<_>>()})
            }
            Ty::Param(n) => json!({"k": "param", "n": n}),
            Ty::Var(_) | Ty::Error => json!({"k": "any"}),
        }
    }

    fn meta(&self) -> String {
        let prog = self.prog;
        let types: Vec<Json> = prog
            .types
            .iter()
            .map(|t| {
                let variants: Vec<Json> = (0..t.variant_count())
                    .map(|v| {
                        let fields = t.variant_fields(v).unwrap_or(&[]);
                        json!({
                            "name": t.variant_name(v),
                            "fields": fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
                            "tys": fields.iter().map(|f| self.ty_json(&f.ty)).collect::<Vec<_>>(),
                        })
                    })
                    .collect();
                json!({
                    "name": t.name,
                    "params": t.params,
                    "record": matches!(t.kind, TypeKind::Record(_)),
                    "variants": variants,
                })
            })
            .collect();
        let mut fns = serde_json::Map::new();
        for d in &prog.fns {
            if !d.is_prelude && d.body.is_some() {
                fns.insert(
                    d.name.clone(),
                    json!({"params": d.params.len(), "ret": self.ty_json(&d.ret)}),
                );
            }
        }
        json!({
            "strs": self.strs,
            "bigs": self.bigs,
            "types": types,
            "fns": fns,
            "option": self.option_ty,
            "objMax": self.a,
        })
        .to_string()
    }

    fn assemble(&mut self, start: u32, exports: &[(String, u32)]) -> Vec<u8> {
        let mut m = WModule::new();

        // ---- types
        let mut types = TypeSection::new();
        let field = |t: StorageType, mutable: bool| FieldType {
            element_type: t,
            mutable,
        };
        let i32f = field(StorageType::Val(ValType::I32), false);
        let anyf = field(StorageType::Val(ANY), false);
        // objects: a subtype chain
        for k in 0..=self.a {
            let mut fields = vec![i32f, i32f];
            fields.extend((0..k).map(|_| anyf));
            let sup = if k == 0 {
                vec![]
            } else {
                vec![self.t_obj(k - 1)]
            };
            types.ty().subtype(&SubType {
                is_final: false,
                supertype_idxs: sup,
                composite_type: CompositeType {
                    inner: CompositeInnerType::Struct(StructType {
                        fields: fields.into(),
                    }),
                    shared: false,
                    descriptor: None,
                    describes: None,
                },
            });
        }
        types
            .ty()
            .struct_([field(StorageType::Val(ValType::F64), false)]);
        types.ty().array(&StorageType::Val(ANY), false);
        for k in 0..=self.c {
            let mut params = vec![ref_null_to(self.t_env())];
            params.extend((0..k).map(|_| ANY));
            types.ty().function(params, [ANY]);
        }
        for k in 0..=self.c {
            types.ty().struct_([
                field(StorageType::Val(ref_to(self.t_fn(k))), false),
                field(StorageType::Val(ref_null_to(self.t_env())), false),
            ]);
        }
        for n in 0..=self.n {
            types
                .ty()
                .function((0..n).map(|_| ANY).collect::<Vec<_>>(), [ANY]);
        }
        for s in self.sigs.clone() {
            match s {
                Sig::Any(n) => types
                    .ty()
                    .function((0..n).map(|_| ANY).collect::<Vec<_>>(), [ANY]),
                Sig::AnyToI32(n) => {
                    types
                        .ty()
                        .function((0..n).map(|_| ANY).collect::<Vec<_>>(), [ValType::I32]);
                }
                Sig::I64Any => types.ty().function([ValType::I64], [ANY]),
                Sig::I32Any => types.ty().function([ValType::I32], [ANY]),
                Sig::AnyI32Any => types.ty().function([ANY, ValType::I32], [ANY]),
                Sig::FailI32 => types.ty().function([ValType::I32], []),
                Sig::F64Any => types.ty().function([ValType::F64], [ANY]),
                Sig::AnyF64 => types.ty().function([ANY], [ValType::F64]),
            }
        }
        // the `() -> ()` type of the start function
        types.ty().function([], []);
        m.section(&types);

        // ---- imports
        let mut imports = ImportSection::new();
        for (name, sig) in &self.imports {
            imports.import("env", name, EntityType::Function(self.t_sig(sig)));
        }
        m.section(&imports);

        // ---- functions
        let mut fsec = FunctionSection::new();
        for (ty, _) in &self.funcs {
            fsec.function(*ty);
        }
        m.section(&fsec);

        // ---- globals
        let mut gsec = GlobalSection::new();
        for _ in 0..self.n_globals {
            gsec.global(
                GlobalType {
                    val_type: ANY,
                    mutable: true,
                    shared: false,
                },
                &ConstExpr::ref_null(HeapType::Abstract {
                    shared: false,
                    ty: AbstractHeapType::None,
                }),
            );
        }
        m.section(&gsec);

        // ---- exports
        let mut esec = ExportSection::new();
        for (name, idx) in exports {
            esec.export(name, ExportKind::Func, *idx);
        }
        m.section(&esec);
        m.section(&StartSection {
            function_index: start,
        });

        // ---- declared function references
        let mut elems = ElementSection::new();
        let declared: Vec<u32> = self.declared.iter().copied().collect();
        if !declared.is_empty() {
            elems.declared(Elements::Functions(declared.into()));
        }
        m.section(&elems);

        // ---- code
        let mut code = CodeSection::new();
        for (_, body) in &self.funcs {
            if let Some(b) = body {
                code.function(b);
            } else {
                let mut f = Function::new([]);
                f.instruction(&I::Unreachable);
                f.instruction(&I::End);
                code.function(&f);
            }
        }
        m.section(&code);
        m.section(&CustomSection {
            name: "tez-meta".into(),
            data: self.meta().into_bytes().into(),
        });
        m.finish()
    }
}
