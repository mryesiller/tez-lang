//! The replay backend (what the browser uses) must reach exactly the verdicts of the process backend.
//! Here the "host" is a native `z3` fed one script at a time, like the browser's asynchronous WebAssembly solver.
#![allow(clippy::unwrap_used, clippy::panic)]

use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};

use tez_syntax::{ParseOptions, parse};
use tez_types::check;
use tez_verify::{Backend, VerifyOptions, replay_set_answers, replay_take_missing, verify};

fn z3_available() -> bool {
    Command::new("z3").arg("--version").output().is_ok()
}

fn solve(query: &str) -> String {
    let mut child = Command::new("z3")
        .args(["-in", "-smt2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(query.as_bytes())
        .unwrap();
    String::from_utf8(child.wait_with_output().unwrap().stdout).unwrap()
}

fn options(backend: Backend) -> VerifyOptions {
    VerifyOptions {
        timeout_ms: 6000,
        include_prelude: false,
        cache_dir: None,
        backend,
    }
}

/// (code, line) of each problem, sorted.
fn summarize(src: &str, problems: &[tez_syntax::Problem]) -> Vec<(String, u32)> {
    let index = tez_syntax::LineIndex::new(src);
    let mut v: Vec<(String, u32)> = problems
        .iter()
        .map(|p| {
            let d = p.to_diagnostic(&index);
            (format!("{:?}", d.code), d.span.start.line)
        })
        .collect();
    v.sort();
    v
}

fn replay(program: &tez_types::hir::Program, src: &str) -> (Vec<tez_syntax::Problem>, usize) {
    let mut answers: HashMap<String, String> = HashMap::new();
    for round in 1..=60 {
        replay_set_answers(answers.clone());
        let result = verify(program, src, &options(Backend::Replay));
        let missing = replay_take_missing();
        if missing.is_empty() {
            return (result.unwrap(), round);
        }
        for m in missing {
            let raw = solve(&m.query);
            answers.insert(m.id, raw);
        }
    }
    panic!("replay did not converge");
}

#[test]
fn replay_matches_the_process_backend_on_every_example() {
    if !z3_available() {
        eprintln!("z3 not installed: skipping");
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples");
    let mut compared = 0;
    let mut rounds_max = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "tez") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let parsed = parse(&src, ParseOptions::default());
        if !parsed.problems.is_empty() {
            continue;
        }
        let out = check(&parsed.file, &src);
        if !out.problems.is_empty() {
            continue; // rejected before verification: nothing to compare
        }
        let native = verify(&out.program, &src, &options(Backend::Process)).unwrap();
        let (replayed, rounds) = replay(&out.program, &src);
        assert_eq!(
            summarize(&src, &native),
            summarize(&src, &replayed),
            "{}: replay disagrees with the process backend",
            path.display()
        );
        compared += 1;
        rounds_max = rounds_max.max(rounds);
    }
    assert!(compared >= 30, "only {compared} programs compared");
    eprintln!("compared {compared} programs; at most {rounds_max} rounds");
}

#[test]
fn replay_finds_the_same_counterexamples_and_rejects_the_same_programs() {
    if !z3_available() {
        eprintln!("z3 not installed: skipping");
        return;
    }
    for src in [
        "fn f(xs: List[Int], i: Int) -> Int = xs.get(i)\n",
        "fn f(a: Int, b: Int) -> Int = a / b\n",
        "fn f(n: Int) -> Int\n  ensures result > n\n  = n + 1\n\nfn g(n: Int) -> Int\n  ensures result > n\n  = n\n",
        "fn loop(n: Int) -> Int\n  decreases n\n  = loop(n + 1)\n",
    ] {
        let parsed = parse(src, ParseOptions::default());
        let out = check(&parsed.file, src);
        assert!(out.problems.is_empty(), "{:?}", out.problems);
        let native = verify(&out.program, src, &options(Backend::Process)).unwrap();
        let (replayed, _) = replay(&out.program, src);
        assert!(
            !native.is_empty(),
            "the native backend should reject:\n{src}"
        );
        assert_eq!(summarize(src, &native), summarize(src, &replayed), "{src}");
        let index = tez_syntax::LineIndex::new(src);
        let cx = |ps: &[tez_syntax::Problem]| {
            ps.iter()
                .map(|p| p.to_diagnostic(&index).counterexample.is_some())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            cx(&native),
            cx(&replayed),
            "counterexamples present in both for:\n{src}"
        );
    }
}

#[test]
fn without_answers_nothing_is_ever_accepted() {
    // a pending run (no solver answers yet) must never look like success
    replay_set_answers(HashMap::new());
    let src =
        "fn f(xs: List[Int], i: Int) -> Int\n  requires 0 <= i && i < xs.len()\n  = xs.get(i)\n";
    let parsed = parse(src, ParseOptions::default());
    let out = check(&parsed.file, src);
    let r = verify(&out.program, src, &options(Backend::Replay));
    let missing = replay_take_missing();
    assert!(!missing.is_empty(), "the verifier must ask for answers");
    assert!(
        r.map_or(true, |ps| !ps.is_empty()),
        "an unanswered run must not verify"
    );
}
