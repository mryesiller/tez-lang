//! Static verification: every obligation the language promises to discharge is proved by an
//! SMT solver, or the program is rejected.
//!
//! The solver is Z3 driven over SMT-LIB. An obligation is *proved* when its negation is
//! unsatisfiable, *refuted* (with a counterexample) when satisfiable, and *unproven* when the
//! solver gives up. Refuted and unproven obligations are both errors.

mod enc;
mod vc;
mod z3;

pub use z3::{
    Backend, CACHE_HITS, Missing, default_cache_dir, replay_set_answers, replay_take_missing,
};

use tez_diag::Code;
use tez_syntax::Problem;
use tez_types::hir::Program;

use crate::vc::{Kind, Vc};
use crate::z3::{Verdict, Z3};

/// Verification settings.
#[derive(Debug, Clone)]
pub struct VerifyOptions {
    /// Per-obligation solver timeout.
    pub timeout_ms: u32,
    /// Also verify the prelude's own function bodies (a self-test of the trusted base).
    pub include_prelude: bool,
    /// Directory remembering proved queries by script hash (`None` = no cache). See [`z3::default_cache_dir`].
    pub cache_dir: Option<std::path::PathBuf>,
    /// Native `z3` process, or script recording for a host that solves asynchronously (the browser).
    pub backend: Backend,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        Self {
            timeout_ms: 8000,
            include_prelude: false,
            cache_dir: z3::default_cache_dir(),
            backend: Backend::default(),
        }
    }
}

fn code_for(kind: Kind) -> Code {
    match kind {
        Kind::DivZero => Code::DivZero,
        Kind::FloatDomain => Code::FloatDomain,
        Kind::IndexBounds => Code::IndexBounds,
        Kind::Precondition => Code::Precondition,
        Kind::Postcondition => Code::Postcondition,
        Kind::Refinement => Code::Refinement,
        Kind::Assert => Code::Assert,
        Kind::Termination => Code::Termination,
    }
}

/// Verifies every function, constant and type-level refinement of `prog`.
///
/// `src` is the user's source text (used to quote contracts in messages).
///
/// # Errors
/// Returns a message when the solver cannot be started or misbehaves. A program must be
/// rejected in that case: nothing was proved.
pub fn verify(prog: &Program, src: &str, opts: &VerifyOptions) -> Result<Vec<Problem>, String> {
    let mut vc = Vc::new(prog, src);
    vc.verify_type_preds();
    vc.verify_consts();
    let first = vc.obligs.len();
    // components of the call graph, callees first; each owns a range of obligations
    let mut groups: Vec<(Vec<usize>, std::ops::Range<usize>)> = vec![(Vec::new(), 0..first)];
    for comp in vc.scc_order() {
        let fns: Vec<usize> = comp
            .into_iter()
            .filter(|&f| {
                prog.fns[f].body.is_some() && (!prog.fns[f].is_prelude || opts.include_prelude)
            })
            .collect();
        if fns.is_empty() {
            continue;
        }
        let start = vc.obligs.len();
        for &f in &fns {
            vc.verify_fn(f);
        }
        groups.push((fns, start..vc.obligs.len()));
    }
    if vc.obligs.is_empty() {
        return Ok(Vec::new());
    }
    let (base, axioms, lit_facts) = vc.enc.parts();
    let mut facts = vc.facts.clone();
    let n_vc = facts.len();
    facts.extend(lit_facts);
    let slicer = Slicer::new(&base, &axioms, &facts, n_vc);

    if let Some(path) = std::env::var_os("TEZ_DUMP_SMT") {
        let _ = std::fs::write(path, slicer.dump(&base, &vc.obligs));
    }

    let mut z3 = Z3::open(opts.backend, opts.timeout_ms, opts.cache_dir.clone())?;
    z3.run(&base)?;
    // The axioms must be consistent: an inconsistent set would "prove" everything.
    // (a quick check: inconsistencies are found at once, "unknown" is the normal answer otherwise)
    let mut check = vec![
        "(push 1)".to_string(),
        "(set-option :timeout 300)".to_string(),
    ];
    check.extend(axioms.iter().cloned());
    z3.run(&check)?;
    let consistent = z3.check_sat()?;
    z3.run(&[
        "(pop 1)".to_string(),
        format!("(set-option :timeout {})", opts.timeout_ms),
    ])?;
    if consistent == Verdict::Proved {
        return Err("internal error: the verifier's axioms are inconsistent".into());
    }

    let mut problems: Vec<Problem> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut report = |o: &crate::vc::Obligation,
                      verdict: &Verdict,
                      ce: Option<Vec<(String, String)>>| {
        if *verdict == Verdict::Proved || !seen.insert((o.span.start, o.span.end, o.kind as u8)) {
            return;
        }
        let mut p = match verdict {
            Verdict::Refuted => Problem::new(code_for(o.kind), o.msg.clone(), o.span),
            _ => Problem::new(Code::Unproven, format!("cannot prove: {}", o.msg), o.span),
        };
        if let Some(e) = &o.expected {
            p = p.expected(e.clone());
        }
        p.counterexample = ce;
        problems.push(p);
    };

    for (fns, range) in groups {
        // termination first: the definitions of recursive functions may only be used afterwards
        let (term, rest): (Vec<usize>, Vec<usize>) =
            range.partition(|&i| vc.obligs[i].kind == Kind::Termination);
        let mut all_proved = true;
        for i in term {
            let o = &vc.obligs[i];
            let (verdict, ce) = solve_one(&mut z3, &slicer, o)?;
            all_proved &= verdict == Verdict::Proved;
            report(o, &verdict, ce);
        }
        if all_proved {
            let flags: Vec<String> = fns
                .iter()
                .filter_map(|f| vc.term_flags.get(f))
                .map(|fl| format!("(assert {fl})"))
                .collect();
            z3.run(&flags)?;
        }
        for i in rest {
            let o = &vc.obligs[i];
            let (verdict, ce) = solve_one(&mut z3, &slicer, o)?;
            report(o, &verdict, ce);
        }
    }
    problems.sort_by_key(|p| p.span.start);
    Ok(problems)
}

type Counterexample = Option<Vec<(String, String)>>;

/// Decides one obligation; on refutation also extracts values for the watched terms.
fn solve_one(
    z3: &mut Z3,
    slicer: &Slicer<'_>,
    o: &crate::vc::Obligation,
) -> Result<(Verdict, Counterexample), String> {
    if o.goal == "true" {
        return Ok((Verdict::Proved, None));
    }
    let (slice_facts, slice_axioms) = slicer.slice(o);
    let with_axioms = build_query(&slice_axioms, &slice_facts, o);
    z3.run(&with_axioms)?;
    let mut verdict = z3.check_sat()?;
    let mut counterexample = None;
    if verdict == Verdict::Refuted {
        counterexample = values(z3, o);
    }
    z3.run(&["(pop 1)".to_string()])?;
    if verdict == Verdict::Unknown {
        // The quantified axioms may keep the solver from answering `sat`. Ask again without
        // them: a model found that way is only a candidate, but it names the failure.
        let without = build_query(&[], &slice_facts, o);
        z3.run(&without)?;
        match z3.check_sat()? {
            Verdict::Refuted => {
                verdict = Verdict::Refuted;
                counterexample = values(z3, o);
            }
            Verdict::Proved => verdict = Verdict::Proved,
            Verdict::Unknown => {}
        }
        z3.run(&["(pop 1)".to_string()])?;
    }
    Ok((verdict, counterexample))
}

fn build_query(axioms: &[String], facts: &[String], o: &crate::vc::Obligation) -> Vec<String> {
    let mut q = vec!["(push 1)".to_string()];
    q.extend(axioms.iter().cloned());
    q.extend(facts.iter().map(|f| format!("(assert {f})")));
    q.extend(o.path.iter().map(|p| format!("(assert {p})")));
    q.push(format!("(assert (not {}))", o.goal));
    q
}

fn values(z3: &mut Z3, o: &crate::vc::Obligation) -> Counterexample {
    if o.watch.is_empty() {
        return None;
    }
    let terms: Vec<String> = o.watch.iter().map(|(_, t)| t.clone()).collect();
    let vals = z3.get_values(&terms).ok()?;
    Some(o.watch.iter().map(|(n, _)| n.clone()).zip(vals).collect())
}

/// Relevance slicing: an obligation only needs the facts connected to it through shared
/// constants, and the axioms for the symbols those formulas mention. Dropping assumptions
/// can only make a proof harder, never unsound.
struct Slicer<'a> {
    facts: &'a [String],
    n_vc: usize,
    axioms: &'a [String],
    consts: std::collections::HashSet<String>,
    by_const: std::collections::HashMap<String, Vec<usize>>,
    axiom_tags: Vec<Vec<String>>,
}

const KEYWORDS: [&str; 14] = [
    "assert", "forall", "ite", "and", "or", "not", "distinct", "true", "false", "Int", "Bool",
    "pattern", "exists", "let",
];

fn idents(s: &str) -> Vec<&str> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_alphabetic() || b[i] == b'_' {
            let st = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(&s[st..i]);
        } else {
            i += 1;
        }
    }
    out
}

impl<'a> Slicer<'a> {
    fn new(base: &[String], axioms: &'a [String], facts: &'a [String], n_vc: usize) -> Self {
        let mut consts = std::collections::HashSet::new();
        for l in base {
            if let Some(r) = l.strip_prefix("(declare-const ") {
                if let Some(n) = r.split_whitespace().next() {
                    consts.insert(n.to_string());
                }
            } else if let Some(r) = l.strip_prefix("(declare-fun ") {
                let mut parts = r.splitn(2, ' ');
                if let (Some(n), Some(rest)) = (parts.next(), parts.next())
                    && rest.starts_with("()")
                {
                    consts.insert(n.to_string());
                }
            }
        }
        let mut by_const: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, f) in facts.iter().enumerate() {
            let mut seen = std::collections::HashSet::new();
            for t in idents(f) {
                if consts.contains(t) && seen.insert(t) {
                    by_const.entry(t.to_string()).or_default().push(i);
                }
            }
        }
        let axiom_tags = axioms
            .iter()
            .map(|a| {
                let tags: Vec<String> = if let Some(p) = a.find(":pattern") {
                    // heads of the trigger terms
                    let rest = &a[p + 8..];
                    let mut heads = Vec::new();
                    let mut depth = 0;
                    let mut expect_head = false;
                    for (i, c) in rest.char_indices() {
                        match c {
                            '(' => {
                                depth += 1;
                                if depth == 2 {
                                    expect_head = true;
                                }
                            }
                            ')' => depth -= 1,
                            _ => {
                                if expect_head && (c.is_ascii_alphabetic() || c == '_') {
                                    let head = idents(&rest[i..]).first().copied().unwrap_or("");
                                    heads.push(head.to_string());
                                    expect_head = false;
                                }
                            }
                        }
                        if depth == 0 && i > 0 {
                            break;
                        }
                    }
                    heads
                } else {
                    idents(a)
                        .into_iter()
                        .filter(|t| !KEYWORDS.contains(t))
                        .map(str::to_string)
                        .collect()
                };
                tags
            })
            .collect();
        Self {
            facts,
            n_vc,
            axioms,
            consts,
            by_const,
            axiom_tags,
        }
    }

    fn slice(&self, o: &crate::vc::Obligation) -> (Vec<String>, Vec<String>) {
        let mut included = vec![false; self.facts.len()];
        let mut queue: Vec<String> = Vec::new();
        let mut seen_c: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut tokens: std::collections::HashSet<String> = std::collections::HashSet::new();
        let seed = o
            .path
            .iter()
            .map(String::as_str)
            .chain(std::iter::once(o.goal.as_str()));
        for t in seed {
            for id in idents(t) {
                tokens.insert(id.to_string());
                if self.consts.contains(id) && seen_c.insert(id.to_string()) {
                    queue.push(id.to_string());
                }
            }
        }
        while let Some(c) = queue.pop() {
            let Some(idx) = self.by_const.get(&c) else {
                continue;
            };
            for &i in idx {
                // facts created after this obligation are not assumptions here;
                // literal facts (appended after all of them) hold everywhere
                if (i >= o.facts_len && i < self.n_vc) || included[i] {
                    continue;
                }
                included[i] = true;
                for id in idents(&self.facts[i]) {
                    tokens.insert(id.to_string());
                    if self.consts.contains(id) && seen_c.insert(id.to_string()) {
                        queue.push(id.to_string());
                    }
                }
            }
        }
        // axioms: those whose trigger symbols occur; axioms add their own symbols (fixpoint)
        let mut ax_in = vec![false; self.axioms.len()];
        loop {
            let mut changed = false;
            for (i, tags) in self.axiom_tags.iter().enumerate() {
                if !ax_in[i] && tags.iter().any(|t| tokens.contains(t)) {
                    ax_in[i] = true;
                    changed = true;
                    for id in idents(&self.axioms[i]) {
                        tokens.insert(id.to_string());
                    }
                }
            }
            if !changed {
                break;
            }
        }
        let f: Vec<String> = included
            .iter()
            .enumerate()
            .filter(|(_, b)| **b)
            .map(|(i, _)| self.facts[i].clone())
            .collect();
        let a: Vec<String> = ax_in
            .iter()
            .enumerate()
            .filter(|(_, b)| **b)
            .map(|(i, _)| self.axioms[i].clone())
            .collect();
        (f, a)
    }

    fn dump(&self, base: &[String], obligs: &[crate::vc::Obligation]) -> String {
        let mut out = vec![
            "(set-option :print-success true)".to_string(),
            "(set-option :timeout 4000)".to_string(),
        ];
        out.extend(base.iter().cloned());
        for (i, o) in obligs.iter().enumerate() {
            let (f, a) = self.slice(o);
            out.push(format!("; obligation {i}: {:?} {}", o.kind, o.msg));
            out.push("(push 1)".into());
            out.extend(a);
            out.extend(f.iter().map(|x| format!("(assert {x})")));
            out.extend(o.path.iter().map(|p| format!("(assert {p})")));
            out.push(format!("(assert (not {}))", o.goal));
            out.push("(check-sat)".into());
            out.push("(pop 1)".into());
        }
        out.join("\n")
    }
}
