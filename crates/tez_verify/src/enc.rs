//! SMT-LIB encoding: sorts, datatypes, and the trusted axioms for built-in types.
//!
//! Everything declared or asserted here is *assumed* by the verifier, so each axiom must be
//! true of Tez's real run-time semantics.

use std::collections::{BTreeMap, HashMap};

use tez_types::Ty;
use tez_types::hir::{Program, TypeKind};

struct CtorDef {
    name: String,
    fields: Vec<(String, String)>,
}

struct DtDef {
    name: String,
    ctors: Vec<CtorDef>,
}

/// Names of the SMT functions that model a `List[T]` sort.
#[derive(Clone)]
pub(crate) struct ListOps {
    pub len: String,
    pub get: String,
    pub nil: String,
    pub snoc: String,
    pub take: String,
    pub drop: String,
    pub concat: String,
    pub rev: String,
    pub range: Option<String>,
    pub rep: String,
}

pub(crate) struct Enc<'p> {
    prog: &'p Program,
    memo: HashMap<Ty, String>,
    uninterp: Vec<String>,
    datatypes: Vec<DtDef>,
    decls: Vec<String>,
    axioms: Vec<String>,
    counter: usize,
    float_lits: BTreeMap<u64, String>,
    str_lits: BTreeMap<String, String>,
    fn_syms: HashMap<String, String>,
    lists: HashMap<String, ListOps>,
    float_ready: bool,
    str_ready: bool,
    ranks: HashMap<String, String>,
}

fn quant(vars: &[(&str, &str)], pattern: Option<&str>, body: &str) -> String {
    let vs: Vec<String> = vars.iter().map(|(n, s)| format!("({n} {s})")).collect();
    match pattern {
        Some(p) => format!(
            "(assert (forall ({}) (! {body} :pattern ({p}))))",
            vs.join(" ")
        ),
        None => format!("(assert (forall ({}) {body}))", vs.join(" ")),
    }
}

impl<'p> Enc<'p> {
    pub fn new(prog: &'p Program) -> Self {
        Self {
            prog,
            memo: HashMap::new(),
            uninterp: Vec::new(),
            datatypes: Vec::new(),
            decls: Vec::new(),
            axioms: Vec::new(),
            counter: 0,
            float_lits: BTreeMap::new(),
            str_lits: BTreeMap::new(),
            fn_syms: HashMap::new(),
            lists: HashMap::new(),
            float_ready: false,
            str_ready: false,
            ranks: HashMap::new(),
        }
    }

    pub fn next_id(&mut self) -> usize {
        self.counter += 1;
        self.counter
    }

    pub fn declare_const(&mut self, prefix: &str, sort: &str) -> String {
        let n = self.next_id();
        let name = format!("{prefix}{n}");
        self.decls.push(format!("(declare-const {name} {sort})"));
        name
    }

    pub fn push_decl(&mut self, d: String) {
        self.decls.push(d);
    }

    /// Like [`Enc::fn_sym`], also telling whether the symbol was just created.
    pub fn fn_sym_new(&mut self, key: &str, args: &[String], ret: &str) -> (String, bool) {
        let fresh = !self.fn_syms.contains_key(key);
        (self.fn_sym(key, args, ret), fresh)
    }

    pub fn add_axiom(&mut self, a: String) {
        self.axioms.push(a);
    }

    /// `forall` over named binders with `lhs` as the trigger.
    pub fn quant_owned(binders: &[(String, String)], lhs: &str, body: &str) -> String {
        let vs: Vec<(&str, &str)> = binders
            .iter()
            .map(|(n, s)| (n.as_str(), s.as_str()))
            .collect();
        quant(&vs, Some(lhs), body)
    }

    /// An uninterpreted function symbol, declared once per `key`.
    pub fn fn_sym(&mut self, key: &str, args: &[String], ret: &str) -> String {
        if let Some(n) = self.fn_syms.get(key) {
            return n.clone();
        }
        let n = self.next_id();
        let name = format!("F{n}");
        self.decls
            .push(format!("(declare-fun {name} ({}) {ret})", args.join(" ")));
        self.fn_syms.insert(key.to_string(), name.clone());
        name
    }

    // ------------------------------------------------------------ sorts

    pub fn sort(&mut self, ty: &Ty) -> String {
        if let Some(s) = self.memo.get(ty) {
            return s.clone();
        }
        match ty {
            Ty::Int | Ty::Var(_) | Ty::Error => "Int".into(),
            Ty::Bool => "Bool".into(),
            Ty::Float => {
                self.ensure_float();
                "Flt".into()
            }
            Ty::Str => {
                self.ensure_str();
                "Str".into()
            }
            Ty::Unit => {
                let name = "Unit".to_string();
                self.memo.insert(ty.clone(), name.clone());
                self.datatypes.push(DtDef {
                    name: name.clone(),
                    ctors: vec![CtorDef {
                        name: "Unit_c0".into(),
                        fields: Vec::new(),
                    }],
                });
                name
            }
            Ty::Param(p) => {
                let name = format!("P_{p}");
                self.memo.insert(ty.clone(), name.clone());
                self.uninterp.push(format!("(declare-sort {name} 0)"));
                name
            }
            Ty::List(elem) => self.list_sort(ty, elem),
            Ty::Tuple(ts) => {
                let n = self.next_id();
                let name = format!("Tup{n}");
                self.memo.insert(ty.clone(), name.clone());
                let fields: Vec<(String, String)> = ts
                    .iter()
                    .enumerate()
                    .map(|(j, t)| (format!("{name}_s0_{j}"), self.sort(t)))
                    .collect();
                self.datatypes.push(DtDef {
                    name: name.clone(),
                    ctors: vec![CtorDef {
                        name: format!("{name}_c0"),
                        fields,
                    }],
                });
                name
            }
            Ty::Adt(id, args) => {
                let n = self.next_id();
                let name = format!("A{n}_{}", self.prog.type_name(*id));
                self.memo.insert(ty.clone(), name.clone());
                let def = &self.prog.types[*id];
                let map: Vec<(String, Ty)> = def
                    .params
                    .iter()
                    .cloned()
                    .zip(args.iter().cloned())
                    .collect();
                let variants: Vec<Vec<Ty>> = match &def.kind {
                    TypeKind::Record(fs) => {
                        vec![fs.iter().map(|f| f.ty.subst_params(&map)).collect()]
                    }
                    TypeKind::Adt(vs) => vs
                        .iter()
                        .map(|v| v.fields.iter().map(|f| f.ty.subst_params(&map)).collect())
                        .collect(),
                };
                let mut ctors = Vec::new();
                for (i, tys) in variants.iter().enumerate() {
                    let fields: Vec<(String, String)> = tys
                        .iter()
                        .enumerate()
                        .map(|(j, t)| (format!("{name}_s{i}_{j}"), self.sort(t)))
                        .collect();
                    ctors.push(CtorDef {
                        name: format!("{name}_c{i}"),
                        fields,
                    });
                }
                self.datatypes.push(DtDef {
                    name: name.clone(),
                    ctors,
                });
                name
            }
            Ty::Fn(ps, r) => {
                let n = self.next_id();
                let name = format!("Fn{n}");
                self.memo.insert(ty.clone(), name.clone());
                self.uninterp.push(format!("(declare-sort {name} 0)"));
                let mut args = vec![name.clone()];
                args.extend(ps.iter().map(|p| self.sort(p)));
                let ret = self.sort(r);
                self.decls.push(format!(
                    "(declare-fun {name}_ap ({}) {ret})",
                    args.join(" ")
                ));
                name
            }
        }
    }

    /// A structural size for a data type: strictly larger than every direct sub-value of the
    /// same type. Lets `decreases e` (with `e` a data value) express structural recursion.
    pub fn rank_fn(&mut self, adt: &Ty) -> String {
        let sort = self.sort(adt);
        if let Some(r) = self.ranks.get(&sort) {
            return r.clone();
        }
        let n = self.next_id();
        let name = format!("rank{n}");
        self.ranks.insert(sort.clone(), name.clone());
        self.decls
            .push(format!("(declare-fun {name} ({sort}) Int)"));
        self.axioms.push(quant(
            &[("x", &sort)],
            Some(&format!("({name} x)")),
            &format!("(>= ({name} x) 0)"),
        ));
        let mut axioms = Vec::new();
        if let Some(dt) = self.datatypes.iter().find(|d| d.name == sort) {
            for c in &dt.ctors {
                for (sel, fsort) in &c.fields {
                    if *fsort == sort {
                        axioms.push(quant(
                            &[("x", &sort)],
                            Some(&format!("({name} x)")),
                            &format!(
                                "(=> ((_ is {}) x) (> ({name} x) ({name} ({sel} x))))",
                                c.name
                            ),
                        ));
                    }
                }
            }
        }
        self.axioms.extend(axioms);
        name
    }

    pub fn unit_val(&mut self) -> String {
        self.sort(&Ty::Unit);
        "Unit_c0".into()
    }

    pub fn ctor(&mut self, ty: &Ty, variant: usize) -> String {
        format!("{}_c{variant}", self.sort(ty))
    }

    pub fn selector(&mut self, ty: &Ty, variant: usize, field: usize) -> String {
        format!("{}_s{variant}_{field}", self.sort(ty))
    }

    pub fn ap_fn(&mut self, fn_ty: &Ty) -> String {
        format!("{}_ap", self.sort(fn_ty))
    }

    // ------------------------------------------------------------ Float

    fn ensure_float(&mut self) {
        if self.float_ready {
            return;
        }
        self.float_ready = true;
        self.memo.insert(Ty::Float, "Flt".into());
        self.uninterp.push("(declare-sort Flt 0)".into());
        for f in ["fadd", "fsub", "fmul", "fdiv"] {
            self.decls.push(format!("(declare-fun {f} (Flt Flt) Flt)"));
        }
        self.decls.push("(declare-fun fneg (Flt) Flt)".into());
        self.decls.push("(declare-fun flt (Flt Flt) Bool)".into());
        self.decls.push("(declare-fun fle (Flt Flt) Bool)".into());
        let ab = [("a", "Flt"), ("b", "Flt")];
        let z = self.float_lit(0.0);
        // sign of a difference: a - b >= 0 iff b <= a, > 0 iff b < a, = 0 iff a = b
        // (true for finite IEEE doubles: gradual underflow, and overflow keeps the sign)
        self.axioms.push(quant(
            &ab,
            Some("(fsub a b)"),
            &format!("(= (fle {z} (fsub a b)) (fle b a))"),
        ));
        self.axioms.push(quant(
            &ab,
            Some("(fsub a b)"),
            &format!("(= (flt {z} (fsub a b)) (flt b a))"),
        ));
        self.axioms.push(quant(
            &ab,
            Some("(fsub a b)"),
            &format!("(= (= (fsub a b) {z}) (= a b))"),
        ));
        let a1 = [("a", "Flt")];
        self.axioms.push(quant(
            &a1,
            Some("(fneg a)"),
            &format!("(= (fle {z} (fneg a)) (fle a {z}))"),
        ));
        self.axioms.push(quant(
            &a1,
            Some("(fneg a)"),
            &format!("(= (flt {z} (fneg a)) (flt a {z}))"),
        ));
        self.axioms.push(quant(
            &ab,
            Some("(fle a b)"),
            "(= (fle a b) (or (flt a b) (= a b)))",
        ));
        self.axioms
            .push(quant(&[("a", "Flt")], Some("(flt a a)"), "(not (flt a a))"));
        self.axioms.push(quant(
            &ab,
            Some("(flt a b)"),
            "(or (flt a b) (= a b) (flt b a))",
        ));
        self.axioms.push(quant(
            &ab,
            Some("(flt a b) (flt b a)"),
            "(=> (flt a b) (not (flt b a)))",
        ));
        self.axioms.push(quant(
            &[("a", "Flt"), ("b", "Flt"), ("c", "Flt")],
            Some("(flt a b) (flt b c)"),
            "(=> (and (flt a b) (flt b c)) (flt a c))",
        ));
    }

    /// A constant standing for a float literal (cached per value; `-0.0` is `0.0`).
    pub fn float_lit(&mut self, f: f64) -> String {
        self.ensure_float();
        let f = if f == 0.0 { 0.0 } else { f };
        let key = f.to_bits();
        if let Some(n) = self.float_lits.get(&key) {
            return n.clone();
        }
        let n = self.next_id();
        let name = format!("FL{n}");
        self.decls.push(format!("(declare-const {name} Flt)"));
        self.float_lits.insert(key, name.clone());
        name
    }

    // ------------------------------------------------------------ Str

    fn ensure_str(&mut self) {
        if self.str_ready {
            return;
        }
        self.str_ready = true;
        self.memo.insert(Ty::Str, "Str".into());
        self.uninterp.push("(declare-sort Str 0)".into());
        self.decls.push("(declare-fun slen (Str) Int)".into());
        self.decls
            .push("(declare-fun sconcat (Str Str) Str)".into());
        self.decls.push("(declare-fun slt (Str Str) Bool)".into());
        self.decls.push("(declare-fun sle (Str Str) Bool)".into());
        let ab = [("a", "Str"), ("b", "Str")];
        self.axioms
            .push(quant(&[("a", "Str")], Some("(slen a)"), "(>= (slen a) 0)"));
        self.axioms.push(quant(
            &ab,
            Some("(sconcat a b)"),
            "(= (slen (sconcat a b)) (+ (slen a) (slen b)))",
        ));
        self.axioms.push(quant(
            &ab,
            Some("(sle a b)"),
            "(= (sle a b) (or (slt a b) (= a b)))",
        ));
        self.axioms
            .push(quant(&[("a", "Str")], Some("(slt a a)"), "(not (slt a a))"));
        self.axioms.push(quant(
            &ab,
            Some("(slt a b)"),
            "(or (slt a b) (= a b) (slt b a))",
        ));
        self.axioms.push(quant(
            &ab,
            Some("(slt a b) (slt b a)"),
            "(=> (slt a b) (not (slt b a)))",
        ));
        // the empty string is the only string of length 0
        self.decls.push("(declare-const sempty Str)".into());
        self.axioms.push("(assert (= (slen sempty) 0))".into());
        self.axioms.push(quant(
            &[("a", "Str")],
            Some("(slen a)"),
            "(=> (= (slen a) 0) (= a sempty))",
        ));
    }

    pub fn str_lit(&mut self, s: &str) -> String {
        self.ensure_str();
        if s.is_empty() {
            return "sempty".into();
        }
        if let Some(n) = self.str_lits.get(s) {
            return n.clone();
        }
        let n = self.next_id();
        let name = format!("SL{n}");
        self.decls.push(format!("(declare-const {name} Str)"));
        self.axioms
            .push(format!("(assert (= (slen {name}) {}))", s.chars().count()));
        self.str_lits.insert(s.to_string(), name.clone());
        name
    }

    // ------------------------------------------------------------ List

    fn list_sort(&mut self, ty: &Ty, elem: &Ty) -> String {
        let n = self.next_id();
        let name = format!("L{n}");
        self.memo.insert(ty.clone(), name.clone());
        self.uninterp.push(format!("(declare-sort {name} 0)"));
        let e = self.sort(elem);
        let ops = ListOps {
            len: format!("{name}_len"),
            get: format!("{name}_get"),
            nil: format!("{name}_nil"),
            snoc: format!("{name}_snoc"),
            take: format!("{name}_take"),
            drop: format!("{name}_drop"),
            concat: format!("{name}_concat"),
            rev: format!("{name}_rev"),
            range: (e == "Int").then(|| format!("{name}_range")),
            rep: format!("{name}_rep"),
        };
        let l = name.as_str();
        let d = &mut self.decls;
        d.push(format!("(declare-fun {} ({l}) Int)", ops.len));
        d.push(format!("(declare-fun {} ({l} Int) {e})", ops.get));
        d.push(format!("(declare-const {} {l})", ops.nil));
        d.push(format!("(declare-fun {} ({l} {e}) {l})", ops.snoc));
        d.push(format!("(declare-fun {} ({l} Int) {l})", ops.take));
        d.push(format!("(declare-fun {} ({l} Int) {l})", ops.drop));
        d.push(format!("(declare-fun {} ({l} {l}) {l})", ops.concat));
        d.push(format!("(declare-fun {} ({l}) {l})", ops.rev));
        d.push(format!("(declare-fun {} ({e} Int) {l})", ops.rep));
        if let Some(r) = &ops.range {
            d.push(format!("(declare-fun {r} (Int Int) {l})"));
        }
        let (len, get, nil, snoc) = (&ops.len, &ops.get, &ops.nil, &ops.snoc);
        let (take, drop, concat, rev, rep) =
            (&ops.take, &ops.drop, &ops.concat, &ops.rev, &ops.rep);
        let a = &mut self.axioms;
        a.push(quant(
            &[("l", l)],
            Some(&format!("({len} l)")),
            &format!("(>= ({len} l) 0)"),
        ));
        a.push(format!("(assert (= ({len} {nil}) 0))"));
        a.push(quant(
            &[("l", l)],
            Some(&format!("({len} l)")),
            &format!("(=> (= ({len} l) 0) (= l {nil}))"),
        ));
        a.push(quant(
            &[("l", l), ("x", &e)],
            Some(&format!("({snoc} l x)")),
            &format!("(= ({len} ({snoc} l x)) (+ ({len} l) 1))"),
        ));
        a.push(quant(
            &[("l", l), ("x", &e), ("i", "Int")],
            Some(&format!("({get} ({snoc} l x) i)")),
            &format!(
                "(=> (and (<= 0 i) (<= i ({len} l))) (= ({get} ({snoc} l x) i) (ite (= i ({len} l)) x ({get} l i))))"
            ),
        ));
        a.push(quant(
            &[("l", l), ("n", "Int")],
            Some(&format!("({take} l n)")),
            &format!("(= ({len} ({take} l n)) (ite (<= n 0) 0 (ite (< n ({len} l)) n ({len} l))))"),
        ));
        a.push(quant(
            &[("l", l), ("n", "Int"), ("i", "Int")],
            Some(&format!("({get} ({take} l n) i)")),
            &format!("(=> (and (<= 0 i) (< i ({len} ({take} l n)))) (= ({get} ({take} l n) i) ({get} l i)))"),
        ));
        a.push(quant(
            &[("l", l), ("n", "Int")],
            Some(&format!("({drop} l n)")),
            &format!(
                "(= ({len} ({drop} l n)) (ite (<= n 0) ({len} l) (ite (< n ({len} l)) (- ({len} l) n) 0)))"
            ),
        ));
        a.push(quant(
            &[("l", l), ("n", "Int"), ("i", "Int")],
            Some(&format!("({get} ({drop} l n) i)")),
            &format!(
                "(=> (and (<= 0 i) (< i ({len} ({drop} l n)))) (= ({get} ({drop} l n) i) ({get} l (+ i (ite (<= n 0) 0 n)))))"
            ),
        ));
        a.push(quant(
            &[("a", l), ("b", l)],
            Some(&format!("({concat} a b)")),
            &format!("(= ({len} ({concat} a b)) (+ ({len} a) ({len} b)))"),
        ));
        a.push(quant(
            &[("a", l), ("b", l), ("i", "Int")],
            Some(&format!("({get} ({concat} a b) i)")),
            &format!(
                "(=> (and (<= 0 i) (< i ({len} ({concat} a b)))) (= ({get} ({concat} a b) i) (ite (< i ({len} a)) ({get} a i) ({get} b (- i ({len} a))))))"
            ),
        ));
        a.push(quant(
            &[("l", l)],
            Some(&format!("({rev} l)")),
            &format!("(= ({len} ({rev} l)) ({len} l))"),
        ));
        a.push(quant(
            &[("l", l), ("i", "Int")],
            Some(&format!("({get} ({rev} l) i)")),
            &format!(
                "(=> (and (<= 0 i) (< i ({len} l))) (= ({get} ({rev} l) i) ({get} l (- (- ({len} l) 1) i))))"
            ),
        ));
        a.push(quant(
            &[("x", &e), ("n", "Int")],
            Some(&format!("({rep} x n)")),
            &format!("(= ({len} ({rep} x n)) (ite (>= n 0) n 0))"),
        ));
        a.push(quant(
            &[("x", &e), ("n", "Int"), ("i", "Int")],
            Some(&format!("({get} ({rep} x n) i)")),
            &format!("(=> (and (<= 0 i) (< i ({len} ({rep} x n)))) (= ({get} ({rep} x n) i) x))"),
        ));
        if let Some(r) = &ops.range {
            a.push(quant(
                &[("lo", "Int"), ("hi", "Int")],
                Some(&format!("({r} lo hi)")),
                &format!("(= ({len} ({r} lo hi)) (ite (> hi lo) (- hi lo) 0))"),
            ));
            a.push(quant(
                &[("lo", "Int"), ("hi", "Int"), ("i", "Int")],
                Some(&format!("({get} ({r} lo hi) i)")),
                &format!("(=> (and (<= 0 i) (< i ({len} ({r} lo hi)))) (= ({get} ({r} lo hi) i) (+ lo i)))"),
            ));
        }
        self.lists.insert(name.clone(), ops);
        name
    }

    pub fn list_ops(&mut self, list_ty: &Ty) -> ListOps {
        let s = self.sort(list_ty);
        self.lists[&s].clone()
    }

    // ------------------------------------------------------------ output

    /// `(base, axioms, literal_facts)`:
    /// * `base`: sorts, datatypes and declarations, always sent;
    /// * `axioms`: quantified or global assertions, sent only when relevant;
    /// * `literal_facts`: ordering facts between float literals and distinct string literals.
    pub fn parts(&self) -> (Vec<String>, Vec<String>, Vec<String>) {
        let mut base = self.uninterp.clone();
        if !self.datatypes.is_empty() {
            let names: Vec<String> = self
                .datatypes
                .iter()
                .map(|d| format!("({} 0)", d.name))
                .collect();
            let bodies: Vec<String> = self
                .datatypes
                .iter()
                .map(|d| {
                    let cs: Vec<String> = d
                        .ctors
                        .iter()
                        .map(|c| {
                            let fs: Vec<String> = c
                                .fields
                                .iter()
                                .map(|(sel, s)| format!("({sel} {s})"))
                                .collect();
                            if fs.is_empty() {
                                format!("({})", c.name)
                            } else {
                                format!("({} {})", c.name, fs.join(" "))
                            }
                        })
                        .collect();
                    format!("({})", cs.join(" "))
                })
                .collect();
            base.push(format!(
                "(declare-datatypes ({}) ({}))",
                names.join(" "),
                bodies.join(" ")
            ));
        }
        base.extend(self.decls.iter().cloned());
        let mut facts = Vec::new();
        let mut lits: Vec<(f64, &String)> = self
            .float_lits
            .iter()
            .map(|(b, n)| (f64::from_bits(*b), n))
            .collect();
        lits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for w in lits.windows(2) {
            facts.push(format!("(flt {} {})", w[0].1, w[1].1));
        }
        if self.str_lits.len() > 1 {
            let names: Vec<&str> = self.str_lits.values().map(String::as_str).collect();
            facts.push(format!("(distinct {})", names.join(" ")));
        }
        (base, self.axioms.clone(), facts)
    }
}
