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
    assert!(n >= 40, "expected at least 40 accepted examples, found {n}");
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
        // Json, Time, Fetch and the like are for apps and are documented in spec/language.md (the card has no room)
        if !["Int", "Float", "Str", "List", "Option", "Result"].contains(&module) {
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

fn project(files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tez-proj-{}-{}",
        std::process::id(),
        files.iter().map(|(n, _)| n.len()).sum::<usize>()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    for (name, text) in files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

#[test]
fn the_shop_example_project_works() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples/projects/shop");
    let main = dir.join("main.tez");
    let main = main.to_str().unwrap();
    assert!(tez(&["check", main]).status.success());
    assert_eq!(s(&tez(&["run", main]).stdout).trim(), "(\"$14.00\", 2)");
    let t = s(&tez(&["test", main]).stdout);
    assert!(t.contains("4 passed, 0 failed"), "{t}");
    for f in ["main.tez", "cart.tez", "util/money.tez"] {
        let p = dir.join(f);
        assert!(
            tez(&["fmt", p.to_str().unwrap(), "--check"])
                .status
                .success(),
            "{f} is not formatted"
        );
    }
}

#[test]
fn project_errors_name_the_file_they_are_in() {
    let dir = project(&[
        (
            "main.tez",
            "use \"lib/a.tez\"\n\nfn main() -> Int = double(2)\n",
        ),
        ("lib/a.tez", "fn double(n: Int) -> Int = n * \"x\"\n"),
    ]);
    let out = tez(&["check", "--json", dir.join("main.tez").to_str().unwrap()]);
    assert!(!out.status.success());
    let v: serde_json::Value = serde_json::from_str(&s(&out.stdout)).unwrap();
    assert_eq!(v["diagnostics"][0]["code"], "E_TYPE_MISMATCH");
    assert_eq!(v["diagnostics"][0]["file"], "lib/a.tez");
    assert_eq!(v["diagnostics"][0]["span"]["start"]["line"], 1);
    // human output shows the file's own path and line
    let human = tez(&["check", dir.join("main.tez").to_str().unwrap()]);
    let text = s(&human.stderr);
    assert!(text.contains("lib/a.tez:1:"), "{text}");
    // a single-file program's JSON has no `file` field
    let single = project(&[("one.tez", "fn main() -> Int = \"x\"\n")]);
    let out = tez(&["check", "--json", single.join("one.tez").to_str().unwrap()]);
    assert!(!s(&out.stdout).contains("\"file\""));
}

#[test]
fn imports_are_checked() {
    let cases: [(&[(&str, &str)], &str); 5] = [
        (
            &[("m.tez", "use \"missing.tez\"\nfn main() -> Int = 1\n")],
            "E_USE_NOT_FOUND",
        ),
        (
            &[("m.tez", "use \"/etc/passwd.tez\"\nfn main() -> Int = 1\n")],
            "E_USE_BAD_PATH",
        ),
        (
            &[("m.tez", "use \"../x.tez\"\nfn main() -> Int = 1\n")],
            "E_USE_BAD_PATH",
        ),
        (
            &[
                ("m.tez", "use \"a.tez\"\nfn main() -> Int = 1\n"),
                ("a.tez", "use \"m.tez\"\nfn f() -> Int = 1\n"),
            ],
            "E_USE_CYCLE",
        ),
        (
            &[
                (
                    "m.tez",
                    "use \"a.tez\"\nfn f() -> Int = 2\nfn main() -> Int = 1\n",
                ),
                ("a.tez", "fn f() -> Int = 1\n"),
            ],
            "E_NAME_DUPLICATE",
        ),
    ];
    for (files, code) in cases {
        let dir = project(files);
        let out = tez(&["check", "--json", dir.join("m.tez").to_str().unwrap()]);
        assert!(
            s(&out.stdout).contains(code),
            "expected {code}, got {}",
            s(&out.stdout)
        );
    }
}

#[test]
fn a_file_imported_twice_is_loaded_once() {
    let dir = project(&[
        (
            "m.tez",
            "use \"a.tez\"\nuse \"b.tez\"\nfn main() -> Int = fa() + fb()\n",
        ),
        ("a.tez", "use \"c.tez\"\nfn fa() -> Int = shared()\n"),
        ("b.tez", "use \"c.tez\"\nfn fb() -> Int = shared()\n"),
        ("c.tez", "fn shared() -> Int = 21\n"),
    ]);
    let out = tez(&["run", dir.join("m.tez").to_str().unwrap()]);
    assert_eq!(s(&out.stdout).trim(), "42", "{}", s(&out.stderr));
}

#[test]
fn files_see_only_the_names_of_files_they_use() {
    let lib = "type Shape = Dot | Box(Int)\n\nconst unit: Int = 1\n\nfn area(s: Shape) -> Int = match s {\n  Dot => 0\n  Box(n) => n * n\n}\n";
    // `mid.tez` uses `lib.tez`; `main.tez` uses only `mid.tez`, so lib's names are not visible in main
    let cases: [(&str, &str); 6] = [
        ("fn main() -> Int = area(Dot)", "`area`"),
        ("fn main() -> Int = unit", "`unit`"),
        (
            "fn main() -> Int = match make() {\n  Box(n) => n\n  _ => 0\n}",
            "`Box`",
        ),
        ("fn main() -> Int = [make()].map(area).sum()", "`area`"),
        (
            "fn size(s: Shape) -> Int = 1\nfn main() -> Int = size(make())",
            "`Shape`",
        ),
        (
            "fn main() -> Int = [make()].map(s => area(s)).sum()",
            "`area`",
        ),
    ];
    for (i, (body, name)) in cases.iter().enumerate() {
        let dir = project(&[
            ("lib.tez", lib),
            (
                "mid.tez",
                "use \"lib.tez\"\n\nfn make() -> Shape = Box(4)\n",
            ),
            (
                &format!("vis{i}/../main.tez")[..],
                &format!("use \"mid.tez\"\n\n{body}\n")[..],
            ),
        ]);
        let out = tez(&[
            "check",
            "--json",
            "--no-verify",
            dir.join("main.tez").to_str().unwrap(),
        ]);
        let v: serde_json::Value = serde_json::from_str(&s(&out.stdout)).unwrap();
        let d = &v["diagnostics"][0];
        assert_eq!(d["code"], "E_USE_MISSING", "{body}: {v}");
        assert_eq!(d["file"], "main.tez");
        assert!(d["message"].as_str().unwrap().contains(name), "{body}: {v}");
        assert_eq!(d["fixes"][0]["edits"][0]["replace"], "use \"lib.tez\"\n");
    }
    // with the `use`, the same program is accepted
    let dir = project(&[
        ("lib.tez", lib),
        (
            "mid.tez",
            "use \"lib.tez\"\n\nfn make() -> Shape = Box(4)\n",
        ),
        (
            "ok/../main.tez",
            "use \"mid.tez\"\nuse \"lib.tez\"\n\nfn main() -> Int = area(make()) + unit\n",
        ),
    ]);
    let out = tez(&[
        "check",
        "--no-verify",
        dir.join("main.tez").to_str().unwrap(),
    ]);
    assert!(out.status.success(), "{}", s(&out.stderr));
    // a name from a file that imports this one: the advice is to move it, not to import back (a cycle)
    let dir = project(&[
        (
            "main.tez",
            "use \"a.tez\"\n\ntype Unit2 = U | V\n\nfn main() -> Int = f(U)\n",
        ),
        ("a.tez", "fn f(u: Unit2) -> Int = 1\n"),
    ]);
    let out = tez(&[
        "check",
        "--json",
        "--no-verify",
        dir.join("main.tez").to_str().unwrap(),
    ]);
    let v: serde_json::Value = serde_json::from_str(&s(&out.stdout)).unwrap();
    assert_eq!(v["diagnostics"][0]["code"], "E_USE_MISSING");
    assert_eq!(v["diagnostics"][0]["file"], "a.tez");
    assert!(
        v["diagnostics"][0]["fixes"]
            .as_array()
            .is_none_or(Vec::is_empty),
        "{v}"
    );
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tez-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Like `project`, but in a directory named for the test, so parallel tests never share one.
fn project_in(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = scratch(name);
    for (file, text) in files {
        let p = dir.join(file);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    dir
}

#[test]
fn help_succeeds_and_a_missing_command_or_a_misspelt_flag_does_not() {
    for args in [
        &["--help"][..],
        &["help"],
        &["check", "--help"],
        &["new", "-h"],
    ] {
        let out = tez(args);
        assert_eq!(out.status.code(), Some(0), "{args:?}");
        assert!(s(&out.stdout).contains("USAGE:"), "{args:?}");
    }
    // no command at all is a usage error, printed on stderr
    let none = tez(&[]);
    assert_eq!(none.status.code(), Some(2));
    assert!(s(&none.stderr).contains("USAGE:"));
    // a typo in a flag must not be ignored silently
    let p = temp("flag.tez", "fn main() -> Int = 1\n");
    let p = p.to_str().unwrap();
    for args in [
        &["check", p, "--jsno"][..],
        &["run", p, "--check"],
        &["fmt", p, "--watch"],
        &["new", "x", "--json"],
    ] {
        let out = tez(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            s(&out.stderr).contains("does not take"),
            "{args:?}: {}",
            s(&out.stderr)
        );
    }
}

#[test]
fn new_starts_projects_that_check_test_run_and_are_formatted() {
    for (flag, run_prints) in [(None, Some("20")), (Some("--app"), None)] {
        let dir = scratch(flag.unwrap_or("program"));
        let d = dir.to_str().unwrap();
        let mut args = vec!["new", d];
        args.extend(flag);
        let out = tez(&args);
        assert!(out.status.success(), "{}", s(&out.stderr));
        let main = dir.join("main.tez");
        let m = main.to_str().unwrap();
        let check = tez(&["check", m]);
        assert!(check.status.success(), "{flag:?}: {}", s(&check.stderr));
        assert!(!s(&check.stdout).contains("NOT verified"));
        let t = tez(&["test", m]);
        assert!(t.status.success(), "{flag:?}: {}", s(&t.stdout));
        assert!(
            tez(&["fmt", m, "--check"]).status.success(),
            "{flag:?}: not formatted"
        );
        if let Some(want) = run_prints {
            assert_eq!(s(&tez(&["run", m]).stdout).trim(), want);
        }
        // a second `new` into the same place refuses and leaves the files alone
        let before = std::fs::read_to_string(&main).unwrap();
        std::fs::write(&main, format!("{before}// mine\n")).unwrap();
        let again = tez(&args);
        assert_eq!(again.status.code(), Some(2));
        assert!(
            std::fs::read_to_string(&main)
                .unwrap()
                .ends_with("// mine\n")
        );
    }
    // `new` creates the directory when it does not exist, nested too
    let nested = scratch("nested").join("a").join("b");
    assert!(tez(&["new", nested.to_str().unwrap()]).status.success());
    assert!(nested.join("main.tez").exists());
}

#[test]
fn fmt_takes_a_directory() {
    let dir = project_in(
        "fmtdir",
        &[
            (
                "main.tez",
                "use \"lib/a.tez\"\n\nfn main() -> Int = double(2)\n",
            ),
            ("lib/a.tez", "fn double(n:Int)->Int=n*2\n"),
            ("notes.txt", "not a tez file\n"),
        ],
    );
    let d = dir.to_str().unwrap();
    assert_eq!(tez(&["fmt", d]).status.code(), Some(2)); // needs --check or --write
    let check = tez(&["fmt", d, "--check"]);
    assert_eq!(check.status.code(), Some(1));
    assert!(
        s(&check.stderr).contains("a.tez is not formatted"),
        "{}",
        s(&check.stderr)
    );
    assert!(!s(&check.stderr).contains("main.tez is not formatted"));
    assert!(tez(&["fmt", d, "--write"]).status.success());
    assert!(tez(&["fmt", d, "--check"]).status.success());
    assert_eq!(
        std::fs::read_to_string(dir.join("lib/a.tez")).unwrap(),
        "fn double(n: Int) -> Int = n * 2\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("notes.txt")).unwrap(),
        "not a tez file\n"
    );
    // an empty directory is an error, not a silent success
    let empty = scratch("empty");
    std::fs::create_dir_all(&empty).unwrap();
    assert_eq!(
        tez(&["fmt", empty.to_str().unwrap(), "--check"])
            .status
            .code(),
        Some(2)
    );
}

/// Waits (up to 15 s) until `file` contains `needle`.
fn wait_for(file: &Path, needle: &str) -> bool {
    for _ in 0..150 {
        if std::fs::read_to_string(file).is_ok_and(|t| t.contains(needle)) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    false
}

#[test]
fn watch_runs_again_when_a_file_of_the_project_changes() {
    let dir = project_in(
        "watch",
        &[
            ("main.tez", "use \"lib.tez\"\n\nfn main() -> Int = one()\n"),
            ("lib.tez", "fn one() -> Int = 1\n"),
        ],
    );
    let log = scratch("watch-out.log");
    let err = scratch("watch-err.log");
    let mut child = Command::new(env!("CARGO_BIN_EXE_tez"))
        .args(["check", "--watch", dir.join("main.tez").to_str().unwrap()])
        .stdout(std::fs::File::create(&log).unwrap())
        .stderr(std::fs::File::create(&err).unwrap())
        .spawn()
        .unwrap();
    let ok1 = wait_for(&log, "ok: ");
    // break the *imported* file: the watcher must notice files that are not the entry file
    std::fs::write(dir.join("lib.tez"), "fn one() -> Int = \"x\"\n").unwrap();
    let broke = wait_for(&err, "E_TYPE_MISMATCH");
    std::fs::write(dir.join("lib.tez"), "fn one() -> Int = 2\n").unwrap();
    let fixed = {
        // a second "ok:" line shows up once the project is healthy again
        let mut n = 0;
        for _ in 0..150 {
            n = std::fs::read_to_string(&log)
                .unwrap_or_default()
                .matches("ok: ")
                .count();
            if n >= 2 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        n >= 2
    };
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        ok1,
        "first run printed nothing: {}",
        std::fs::read_to_string(&log).unwrap_or_default()
    );
    assert!(
        broke,
        "the change to lib.tez was not noticed: {}",
        std::fs::read_to_string(&err).unwrap_or_default()
    );
    assert!(fixed, "the fix was not noticed");
}

#[test]
fn prelude_types_leave_names_free_for_programs() {
    // every prelude type and constructor is a name no program can define, so helpers must not add types of their own
    let prelude = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/tez_types/prelude/prelude.tez"),
    )
    .unwrap();
    let types: Vec<&str> = prelude
        .lines()
        .filter_map(|l| l.strip_prefix("type "))
        .filter_map(|l| l.split(['[', ' ']).next())
        .collect();
    assert_eq!(
        types,
        ["Option", "Result", "Html", "Attr", "Sub", "Fetch", "Json"]
    );
    let p = temp(
        "own-token-names.tez",
        "type Tok = TkStr(Str) | TkWord(Str) | FrArr(Int)\n\ntest own = match TkStr(\"x\") {\n  TkStr(s) => s == \"x\"\n  _ => false\n}\n",
    );
    let o = tez(&["test", p.to_str().unwrap()]);
    assert!(o.status.success(), "{}{}", s(&o.stdout), s(&o.stderr));
}

#[test]
fn app_modules_are_documented_in_the_language_reference() {
    let language = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/language.md"),
    )
    .unwrap();
    let prelude = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/tez_types/prelude/prelude.tez"),
    )
    .unwrap();
    // the public functions of the app modules (helpers of the JSON reader are internal)
    let internal = [
        "isSpace",
        "isPunct",
        "stringEnd",
        "wordEnd",
        "hexDigit",
        "hexValue",
        "simpleEscape",
        "escaped",
        "lone",
        "decode",
        "unescape",
        "tokens",
        "word",
        "attach",
        "step",
        "findKey",
        "quote",
        "control",
        "textList",
        "textMembers",
        "pad2",
    ];
    for line in prelude.lines() {
        let Some(rest) = line.strip_prefix("fn ") else {
            continue;
        };
        let Some((module, tail)) = rest.split_once('.') else {
            continue;
        };
        if !["Json", "Time", "Fetch", "Char"].contains(&module) {
            continue;
        }
        let name: String = tail.chars().take_while(|c| c.is_alphanumeric()).collect();
        if internal.contains(&name.as_str()) {
            continue;
        }
        assert!(
            language.contains(&format!("{module}.{name}"))
                || language.contains(&format!(".{name}(")),
            "`{module}.{name}` is not described in spec/language.md"
        );
    }
}

#[test]
fn the_hacker_news_example_site_is_proved_and_its_tests_pass() {
    let main = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/hn-live/main.tez");
    let main = main.to_str().unwrap();
    let check = tez(&["check", main]);
    assert!(check.status.success(), "{}", s(&check.stderr));
    assert!(!s(&check.stdout).contains("NOT verified"));
    let t = s(&tez(&["test", main]).stdout);
    assert!(t.contains("0 failed"), "{t}");
}
