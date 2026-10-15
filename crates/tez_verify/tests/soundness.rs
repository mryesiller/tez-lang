//! Soundness fuzzing: generate random programs with contracts, keep those the verifier
//! accepts, run them in the interpreter with contract checking on random inputs, and fail if
//! an accepted program ever violates a contract, divides by zero or indexes out of range.
//!
//! This is the property the whole language rests on. Rejected programs are not examined
//! (the verifier is allowed to be incomplete).
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use tez_diag::Code;
use tez_interp::{Interp, ListVal, Options, Value, with_big_stack};
use tez_syntax::{ParseOptions, parse};
use tez_types::check;
use tez_verify::{VerifyOptions, verify};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + i64::try_from(self.below(u64::try_from(hi - lo + 1).unwrap())).unwrap()
    }
}

fn gen_int(r: &mut Rng, depth: u32, vars: &[&str], has_xs: bool, result: bool) -> String {
    let leaf = depth == 0 || r.below(4) == 0;
    if leaf {
        return match r.below(4) {
            0 => r.range(-3, 5).to_string().replace('-', "0 - "),
            1 if has_xs => "xs.len()".into(),
            2 if result => "result".into(),
            _ => vars[r.below(vars.len() as u64) as usize].to_string(),
        };
    }
    let a = gen_int(r, depth - 1, vars, has_xs, result);
    let b = gen_int(r, depth - 1, vars, has_xs, result);
    match r.below(11) {
        0 => format!("({a} + {b})"),
        1 => format!("({a} - {b})"),
        2 => format!("({a} * {b})"),
        3 => format!("({a} / {b})"),
        4 => format!("({a} % {b})"),
        5 => format!("({a}).abs()"),
        6 => format!("({a}).min({b})"),
        7 => format!("({a}).max({b})"),
        8 if has_xs => format!("xs[{a}]"),
        _ => {
            let c = gen_bool(r, depth - 1, vars, has_xs, result);
            format!("(if {c} {{ {a} }} else {{ {b} }})")
        }
    }
}

fn gen_bool(r: &mut Rng, depth: u32, vars: &[&str], has_xs: bool, result: bool) -> String {
    if depth == 0 || r.below(3) > 0 {
        let a = gen_int(r, depth.min(1), vars, has_xs, result);
        let b = gen_int(r, depth.min(1), vars, has_xs, result);
        let op = ["<", "<=", "==", "!=", ">", ">="][r.below(6) as usize];
        return format!("({a} {op} {b})");
    }
    let a = gen_bool(r, depth - 1, vars, has_xs, result);
    let b = gen_bool(r, depth - 1, vars, has_xs, result);
    match r.below(3) {
        0 => format!("({a} && {b})"),
        1 => format!("({a} || {b})"),
        _ => format!("(!{a})"),
    }
}

fn program(r: &mut Rng) -> (String, bool) {
    let has_xs = r.below(2) == 0;
    let vars = ["a", "b", "c"];
    let mut src = String::from("fn f(a: Int, b: Int, c: Int");
    if has_xs {
        src.push_str(", xs: List[Int]");
    }
    src.push_str(") -> Int\n");
    for _ in 0..r.below(3) {
        src.push_str(&format!(
            "  requires {}\n",
            gen_bool(r, 1, &vars, has_xs, false)
        ));
    }
    for _ in 0..r.below(3) {
        src.push_str(&format!(
            "  ensures {}\n",
            gen_bool(r, 1, &vars, has_xs, true)
        ));
    }
    src.push_str(&format!("  = {}\n", gen_int(r, 3, &vars, has_xs, false)));
    (src, has_xs)
}

fn violation_code(c: Code) -> bool {
    matches!(
        c,
        Code::DivZero
            | Code::IndexBounds
            | Code::Postcondition
            | Code::Refinement
            | Code::Assert
            | Code::FloatDomain
    )
}

#[test]
fn accepted_programs_never_violate_their_contracts() {
    if std::process::Command::new("z3")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("z3 not installed: skipping");
        return;
    }
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut accepted, mut total, mut runs) = (0u32, 0u32, 0u32);
    let n: u32 = std::env::var("TEZ_FUZZ_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);
    let (mut rejected_violating, mut rejected) = (0u32, 0u32);
    for _ in 0..n {
        let (src, has_xs) = program(&mut rng);
        let parsed = parse(&src, ParseOptions::default());
        if !parsed.problems.is_empty() {
            panic!(
                "generator produced unparsable code:\n{src}\n{:?}",
                parsed.problems
            );
        }
        let checked = check(&parsed.file, &src);
        if !checked.problems.is_empty() {
            panic!(
                "generator produced ill-typed code:\n{src}\n{:?}",
                checked.problems
            );
        }
        total += 1;
        let problems = verify(
            &checked.program,
            &src,
            &VerifyOptions {
                timeout_ms: 2000,
                include_prelude: false,
                cache_dir: None,
            },
        )
        .unwrap_or_else(|e| panic!("verifier error: {e}\n{src}"));
        let verified = problems.is_empty();
        if verified {
            accepted += 1;
        } else {
            rejected += 1;
        }
        let prog = &checked.program;
        let id = prog.find_fn("f").unwrap();
        for _ in 0..40 {
            let ints = [rng.range(-6, 6), rng.range(-6, 6), rng.range(-6, 6)];
            let xs: Option<Vec<i64>> = has_xs.then(|| {
                let n = rng.below(5);
                (0..n).map(|_| rng.range(-5, 5)).collect()
            });
            let opts = Options {
                check_contracts: true,
                ..Options::default()
            };
            let (xs2, ints2) = (xs.clone(), ints);
            let result = with_big_stack(move || {
                let mut args: Vec<Value> = ints2.iter().map(|&i| Value::Int(i)).collect();
                if let Some(v) = &xs2 {
                    args.push(Value::List(ListVal::new(
                        v.iter().map(|&i| Value::Int(i)).collect(),
                    )));
                }
                let mut it = Interp::new(prog, opts);
                it.call(id, args)
                    .map(|v| v.display(prog))
                    .map_err(|e| (e.code, e.message))
            });
            runs += 1;
            if std::env::var_os("TEZ_FUZZ_DEBUG").is_some() && !verified && runs % 7 == 0 {
                eprintln!(
                    "rejected run: {:?}\n{src}",
                    result.as_ref().map_err(|e| e.0)
                );
            }
            if let Err((code, msg)) = result {
                // a violated `requires` just means this input is outside the contract
                if code == Code::Precondition {
                    continue;
                }
                if !verified {
                    if violation_code(code) {
                        rejected_violating += 1;
                        break;
                    }
                    continue;
                }
                assert!(
                    !violation_code(code),
                    "UNSOUND: the verifier accepted a program that fails at run time with {code:?} ({msg})\ninput: {ints:?} {xs:?}\nsource:\n{src}"
                );
            }
        }
    }
    eprintln!(
        "soundness fuzz: {accepted}/{total} programs verified, {runs} runs, no violations; \
         {rejected_violating}/{rejected} rejected programs really do fail when run"
    );
    assert!(
        accepted >= n / 8,
        "the generator should yield verifiable programs ({accepted}/{total})"
    );
    assert!(
        rejected_violating > 0,
        "the fuzzer never saw a violation: it would not catch unsoundness"
    );
}
