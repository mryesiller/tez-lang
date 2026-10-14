//! CLI behaviour and the example corpus: every example must check, be canonically
//! formatted and print its `.expected` output; every `.reject.tez` must be refused with
//! exactly the error code named in its first line.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tez(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_tez"))
        .args(args)
        .output()
        .unwrap()
}

fn examples() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples");
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "tez"))
        .collect();
    v.sort();
    v
}

fn s(o: &[u8]) -> String {
    String::from_utf8_lossy(o).into_owned()
}

fn is_reject(p: &Path) -> bool {
    p.file_name()
        .is_some_and(|n| n.to_string_lossy().ends_with(".reject.tez"))
}

#[test]
fn accepted_examples_check_format_and_run() {
    let mut n = 0;
    for p in examples().iter().filter(|p| !is_reject(p)) {
        let path = p.to_str().unwrap();
        let name = p.file_name().unwrap().to_string_lossy();
        let check = tez(&["check", path]);
        assert!(
            check.status.success(),
            "{name}: check failed\n{}",
            s(&check.stderr)
        );
        let fmt = tez(&["fmt", path, "--check"]);
        assert!(
            fmt.status.success(),
            "{name}: not canonically formatted (run `tez fmt --write`)"
        );
        let want = std::fs::read_to_string(p.with_extension("expected"))
            .unwrap_or_else(|_| panic!("{name}: missing .expected"));
        let run = tez(&["run", path]);
        assert!(
            run.status.success(),
            "{name}: run failed\n{}",
            s(&run.stderr)
        );
        assert_eq!(s(&run.stdout).trim(), want.trim(), "{name}: wrong output");
        n += 1;
    }
    assert!(n >= 14, "expected at least 14 accepted examples, found {n}");
}

#[test]
fn rejected_examples_report_exactly_the_expected_code() {
    let mut n = 0;
    for p in examples().iter().filter(|p| is_reject(p)) {
        let src = std::fs::read_to_string(p).unwrap();
        let want = src
            .lines()
            .next()
            .unwrap()
            .trim_start_matches("// expect: ")
            .trim()
            .to_string();
        let out = tez(&["check", p.to_str().unwrap(), "--json"]);
        assert_eq!(
            out.status.code(),
            Some(1),
            "{}: should be rejected",
            p.display()
        );
        let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(report["status"], "error");
        let codes: Vec<&str> = report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["code"].as_str().unwrap())
            .collect();
        assert_eq!(codes, vec![want.as_str()], "{}", p.display());
        n += 1;
    }
    assert!(n >= 10, "expected at least 10 reject examples, found {n}");
}

fn temp(name: &str, body: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tez-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn version_help_and_grammar() {
    let v = tez(&["--version"]);
    assert!(s(&v.stdout).starts_with("tez "));
    assert!(tez(&["--help"]).status.success());
    let g = tez(&["grammar"]);
    assert!(g.status.success());
    assert!(s(&g.stdout).contains("file        ="));
}

#[test]
fn usage_errors_exit_2() {
    assert_eq!(tez(&["frobnicate"]).status.code(), Some(2));
    assert_eq!(tez(&["check"]).status.code(), Some(2));
    assert_eq!(tez(&["check", "/no/such/file.tez"]).status.code(), Some(2));
}

#[test]
fn human_diagnostics_have_source_excerpt_and_fix() {
    let p = temp("human.tez", "fn f(xs: List[Int]) -> Int = List.len(xs)\n");
    let out = tez(&["check", p.to_str().unwrap()]);
    let err = s(&out.stderr);
    assert_eq!(out.status.code(), Some(1));
    assert!(err.contains("error[E_NAME_USE_METHOD_SYNTAX]"), "{err}");
    assert!(
        err.contains("fn f(xs: List[Int]) -> Int = List.len(xs)"),
        "{err}"
    );
    assert!(err.contains("^"), "{err}");
    assert!(
        err.contains("fix: use method syntax -> `xs.len()`"),
        "{err}"
    );
}

#[test]
fn json_report_for_a_clean_program() {
    let p = temp("clean.tez", "fn main() -> Int = 1\n");
    let out = tez(&["check", p.to_str().unwrap(), "--json"]);
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "ok");
    assert_eq!(v["version"], 1);
}

#[test]
fn run_chooses_function_and_reports_runtime_errors() {
    let p = temp(
        "fns.tez",
        "fn one() -> Int = 1\nfn boom() -> Int = 1 / 0\nfn main() -> Int = 7\n",
    );
    let path = p.to_str().unwrap();
    assert_eq!(s(&tez(&["run", path]).stdout).trim(), "7");
    assert_eq!(s(&tez(&["run", path, "one"]).stdout).trim(), "1");
    let boom = tez(&["run", path, "boom", "--json"]);
    assert_eq!(boom.status.code(), Some(1));
    let v: serde_json::Value = serde_json::from_slice(&boom.stdout).unwrap();
    assert_eq!(v["diagnostics"][0]["code"], "E_DIV_ZERO");
    assert_eq!(v["diagnostics"][0]["phase"], "runtime");
}

#[test]
fn contracts_flag_catches_violations() {
    let p = temp(
        "contract.tez",
        "fn f(x: Int) -> Int requires x > 0 = x\nfn main() -> Int = f(0)\n",
    );
    let path = p.to_str().unwrap();
    assert!(tez(&["run", path]).status.success());
    let checked = tez(&["run", path, "--contracts", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(v["diagnostics"][0]["code"], "E_PRECONDITION");
}

#[test]
fn test_command() {
    let good = temp(
        "good.tez",
        "fn d(x: Int) -> Int = x * 2\ntest ok1 = d(2) == 4\n",
    );
    let out = tez(&["test", good.to_str().unwrap()]);
    assert!(out.status.success(), "{}", s(&out.stdout));
    assert!(s(&out.stdout).contains("1 passed, 0 failed"));
    let bad = temp(
        "bad.tez",
        "fn d(x: Int) -> Int = x * 2\ntest wrong = d(2) == 5\ntest crash = 1 / 0 == 0\n",
    );
    let out = tez(&["test", bad.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1));
    let o = s(&out.stdout);
    assert!(
        o.contains("FAILED  wrong") && o.contains("ERROR   crash"),
        "{o}"
    );
}

#[test]
fn fmt_modes() {
    let p = temp("messy.tez", "fn   f( a:Int )->Int=a+1\n");
    let path = p.to_str().unwrap();
    assert_eq!(
        s(&tez(&["fmt", path]).stdout),
        "fn f(a: Int) -> Int = a + 1\n"
    );
    assert_eq!(tez(&["fmt", path, "--check"]).status.code(), Some(1));
    assert!(tez(&["fmt", path, "--write"]).status.success());
    assert_eq!(
        std::fs::read_to_string(&p).unwrap(),
        "fn f(a: Int) -> Int = a + 1\n"
    );
    assert!(tez(&["fmt", path, "--check"]).status.success());
    let bad = temp("syntax.tez", "fn f( -> Int = 1\n");
    assert_eq!(tez(&["fmt", bad.to_str().unwrap()]).status.code(), Some(1));
}
