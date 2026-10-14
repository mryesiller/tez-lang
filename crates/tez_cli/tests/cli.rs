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
    assert_eq!(s(&tez(&["run", path, "--no-verify"]).stdout).trim(), "7");
    assert_eq!(
        s(&tez(&["run", path, "one", "--no-verify"]).stdout).trim(),
        "1"
    );
    // verified by default: the division by zero in `boom` is refused before anything runs
    assert_eq!(tez(&["run", path]).status.code(), Some(1));
    let boom = tez(&["run", path, "boom", "--no-verify", "--json"]);
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
    assert!(tez(&["run", path, "--no-verify"]).status.success());
    let refused = tez(&["run", path]);
    assert_eq!(
        refused.status.code(),
        Some(1),
        "the verifier must refuse an unprovable precondition"
    );
    let checked = tez(&["run", path, "--no-verify", "--contracts", "--json"]);
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
    let out = tez(&["test", bad.to_str().unwrap(), "--no-verify"]);
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

fn card() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/card.md"))
        .unwrap()
}

#[test]
fn language_card_examples_verify() {
    let card = card();
    let mut src = String::new();
    for block in card.split("```").skip(1).step_by(2) {
        src.push_str(block.trim_start_matches('\n'));
        src.push('\n');
    }
    src.push_str("fn first[T](xs: List[T]) -> Option[T] = match xs {\n  [] => None\n  [h, ..t] => Some(h)\n}\n");
    let p = temp("card.tez", &src);
    let out = tez(&["check", p.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "the card's own examples must verify:\n{}",
        s(&out.stderr)
    );
}

#[test]
fn language_card_lists_exactly_the_prelude_methods() {
    let card = card();
    let prelude = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/tez_types/prelude/prelude.tez"),
    )
    .unwrap();
    for line in card.lines().filter(|l| l.starts_with("- ")) {
        for module in ["Int", "Float", "Str", "List", "Option", "Result"] {
            let Some(rest) = line.split(&format!("{module} `")).nth(1) else {
                continue;
            };
            let Some(list) = rest.split('`').next() else {
                continue;
            };
            if list.contains('.') {
                continue;
            }
            for name in list.split_whitespace() {
                assert!(
                    prelude.contains(&format!("fn {module}.{name}")),
                    "the card lists `{module}.{name}` but the prelude has no such function"
                );
            }
        }
    }
    // and the other way round: every prelude method appears on the card
    for line in prelude.lines() {
        let Some(rest) = line
            .strip_prefix("extern fn ")
            .or_else(|| line.strip_prefix("fn "))
        else {
            continue;
        };
        let Some((module, tail)) = rest.split_once('.') else {
            continue;
        };
        let name: String = tail.chars().take_while(|c| c.is_alphanumeric()).collect();
        if matches!(name.as_str(), "range" | "repeat") && module == "List" {
            continue;
        }
        assert!(
            card.contains(&name),
            "prelude function `{module}.{name}` is missing from spec/card.md"
        );
    }
}

/// Every program in docs/TOUR.md must do what the text says: `tez` blocks verify (and run, when they have a
/// `main`), `tez-reject` blocks fail with exactly the code in their `// expect:` line.
#[test]
fn tour_snippets_are_real() {
    let tour =
        std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/TOUR.md"))
            .unwrap();
    let mut seen = 0;
    for (i, block) in tour.split("```").enumerate().filter(|(i, _)| i % 2 == 1) {
        let (lang, body) = block.split_once('\n').unwrap();
        if lang != "tez" && lang != "tez-reject" {
            continue;
        }
        seen += 1;
        let p = temp(&format!("tour{i}.tez"), body);
        let path = p.to_str().unwrap();
        if lang == "tez-reject" {
            let code = body
                .lines()
                .next()
                .and_then(|l| l.strip_prefix("// expect: "))
                .unwrap();
            let out = tez(&["check", "--json", path]);
            assert!(!out.status.success(), "block {i} should be rejected");
            assert!(
                s(&out.stdout).contains(code),
                "block {i}: expected {code}, got {}",
                s(&out.stdout)
            );
        } else {
            assert!(
                tez(&["check", path]).status.success(),
                "tour block {i} must verify"
            );
            if body.contains("fn main(") {
                let out = tez(&["run", path]);
                assert!(
                    out.status.success(),
                    "tour block {i} must run:\n{}",
                    s(&out.stderr)
                );
            }
        }
    }
    assert!(seen >= 12, "the tour lost its code blocks");
}
