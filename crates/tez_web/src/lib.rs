//! WebAssembly entry points. Every function takes and returns plain strings (JSON), so the
//! JavaScript side stays trivial and the contract is easy to test.
//!
//! Verification needs an SMT solver and is not available in the browser yet; `check` says so
//! explicitly (`"verified": false`) so a UI can never present unproven code as proven.

use serde_json::{Value as Json, json};
use tez_diag::Report;
use tez_driver::{CompileOptions, Compiled, compile_project, render_human_project};
use tez_fmt::{FmtError, format_source};
use tez_interp::{Interp, Options, run_function, run_tests};
use tez_syntax::{ByteSpan, LineIndex, ParseOptions, Problem};
use wasm_bindgen::prelude::*;

fn report_json(report: &Report) -> Json {
    serde_json::to_value(report).unwrap_or(Json::Null)
}

fn to_string(v: &Json) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
}

/// The files of a project as the page or the Node CLI hands them over: `{"path": "text"}` (paths relative to the
/// project directory, `/`-separated). `use` lines are resolved only against this map.
fn file_map(files_json: &str) -> std::collections::HashMap<String, String> {
    serde_json::from_str(files_json).unwrap_or_default()
}

fn compile_web(
    entry: &str,
    files_json: &str,
    options: &CompileOptions,
) -> (Compiled, std::collections::HashMap<String, String>) {
    let files = file_map(files_json);
    let src = files.get(entry).cloned().unwrap_or_default();
    let load = |rel: &str| files.get(rel).cloned();
    (compile_project(entry, &src, &load, options), files)
}

/// Human-readable diagnostics (`dir` is prepended to the names of files other than the entry).
fn human(c: &Compiled, report: &Report, dir: &str, entry: &str) -> String {
    render_human_project(report, &c.files, dir, &format!("{dir}{entry}"))
}

fn no_verify() -> CompileOptions {
    CompileOptions {
        verify: false,
        ..CompileOptions::default()
    }
}

/// A runtime failure as a diagnostic positioned in the file it happened in.
fn runtime_report(c: &Compiled, e: &tez_interp::RuntimeError) -> Report {
    let (k, local) = c.locate(e.span.start);
    let file = &c.files[k];
    let len = e.span.end.saturating_sub(e.span.start);
    let mut d = Problem::new(e.code, e.message.clone(), ByteSpan::new(local, local + len))
        .to_diagnostic(&LineIndex::new(&file.text));
    d.phase = tez_diag::Phase::Runtime;
    if c.files.len() > 1 {
        d.file = Some(file.name.clone());
    }
    Report::new(vec![d])
}

/// Interpreter limits that fit the browser's small stack.
fn web_options(contracts: bool) -> Options {
    Options {
        max_depth: 100_000,
        max_eval_depth: 2500,
        fuel: 200_000_000,
        check_contracts: contracts,
    }
}

/// Version of the compiler.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Parses and type-checks `src`. Returns `{report, verified}`; `verified` is always `false`
/// in the browser.
#[wasm_bindgen]
pub fn check(src: &str) -> String {
    check_project("main.tez", &single(src), "")
}

/// The `files` map of a one-file program.
fn single(src: &str) -> String {
    to_string(&json!({ "main.tez": src }))
}

/// Like [`check`] for a project: `entry` names a file of `files_json` (`{"path": "text"}`), and the files it `use`s
/// are read from that map. `dir` is prepended to file names in the `human` text. Returns `{report, verified, human}`.
#[wasm_bindgen]
pub fn check_project(entry: &str, files_json: &str, dir: &str) -> String {
    let (c, _) = compile_web(entry, files_json, &no_verify());
    to_string(&json!({
        "report": report_json(&c.report),
        "verified": false,
        "human": human(&c, &c.report, dir, entry),
    }))
}

/// One round of in-browser verification.
///
/// The verifier cannot wait for a solver, so it works in rounds (see `tez_verify`'s replay backend): each call
/// takes the solver answers found so far (`answers_json`: `{id: raw solver output}`) and returns either
/// `{done: false, missing: [{id, query, wantsValues}]}` (solve each `query` in a fresh Z3, answer under `id`, call
/// again) or `{done: true, verified, report}` once every query the verifier needed has an answer.
#[wasm_bindgen]
pub fn verify_step(src: &str, answers_json: &str) -> String {
    verify_project_step("main.tez", &single(src), answers_json, "")
}

/// [`verify_step`] for a project (see [`check_project`]); a finished answer also carries `human`.
#[wasm_bindgen]
pub fn verify_project_step(entry: &str, files_json: &str, answers_json: &str, dir: &str) -> String {
    let answers: std::collections::HashMap<String, String> =
        serde_json::from_str(answers_json).unwrap_or_default();
    tez_verify::replay_set_answers(answers);
    let (c, _) = compile_web(
        entry,
        files_json,
        &CompileOptions {
            verify: true,
            backend: tez_verify::Backend::Replay,
        },
    );
    let missing = tez_verify::replay_take_missing();
    if missing.is_empty() {
        let verified = c.report.diagnostics.is_empty();
        return to_string(&json!({
            "done": true,
            "verified": verified,
            "report": report_json(&c.report),
            "human": human(&c, &c.report, dir, entry),
        }));
    }
    let items: Vec<Json> = missing
        .iter()
        .map(|m| json!({ "id": m.id, "query": m.query, "wantsValues": m.wants_values }))
        .collect();
    to_string(&json!({ "done": false, "missing": items }))
}

/// Formats `src` canonically. Returns `{ok: true, text}` or `{ok: false, report | comment}`.
#[wasm_bindgen]
pub fn format(src: &str) -> String {
    let v = match format_source(src, ParseOptions::default()) {
        Ok(text) => json!({ "ok": true, "text": text }),
        Err(FmtError::Syntax(problems)) => {
            let index = LineIndex::new(src);
            let report = Report::new(problems.iter().map(|p| p.to_diagnostic(&index)).collect());
            json!({ "ok": false, "report": report_json(&report) })
        }
        Err(FmtError::Comment(span)) => {
            let pos = LineIndex::new(src).pos(span.start);
            json!({ "ok": false, "comment": { "line": pos.line, "col": pos.col } })
        }
    };
    to_string(&v)
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Compiles `src` to a WasmGC module. Returns `{ok: true, wasm}` (base64) or
/// `{ok: false, report | error}`. Run it with `runtime/tez-runtime.mjs`.
#[wasm_bindgen]
pub fn build(src: &str) -> String {
    build_project("main.tez", &single(src), "")
}

/// [`build`] for a project (see [`check_project`]); a failure also carries `human`.
#[wasm_bindgen]
pub fn build_project(entry: &str, files_json: &str, dir: &str) -> String {
    let (c, _) = compile_web(entry, files_json, &no_verify());
    let Some(prog) = &c.program else {
        return to_string(&json!({
            "ok": false,
            "report": report_json(&c.report),
            "human": human(&c, &c.report, dir, entry),
        }));
    };
    let v = match tez_wasm::compile(prog) {
        Ok(m) => json!({ "ok": true, "wasm": base64(&m.wasm) }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    };
    to_string(&v)
}

/// Runs the zero-argument function `name`. Returns `{ok: true, value}` or `{ok: false, report}`.
#[wasm_bindgen]
pub fn run(src: &str, name: &str, contracts: bool) -> String {
    run_project("main.tez", &single(src), name, contracts, "")
}

/// [`run`] for a project (see [`check_project`]); a failure also carries `human`.
#[wasm_bindgen]
pub fn run_project(
    entry: &str,
    files_json: &str,
    name: &str,
    contracts: bool,
    dir: &str,
) -> String {
    let (c, _) = compile_web(entry, files_json, &no_verify());
    let Some(prog) = &c.program else {
        return to_string(&json!({
            "ok": false,
            "report": report_json(&c.report),
            "human": human(&c, &c.report, dir, entry),
        }));
    };
    let v = match run_function(prog, name, web_options(contracts)) {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(e) => {
            let report = runtime_report(&c, &e);
            json!({ "ok": false, "report": report_json(&report), "human": human(&c, &report, dir, entry) })
        }
    };
    to_string(&v)
}

/// Runs every `test` item. Returns `{ok, results: [{name, outcome: "pass"|"fail"|"error", message?}]}`.
#[wasm_bindgen]
pub fn test(src: &str, contracts: bool) -> String {
    test_project("main.tez", &single(src), contracts, "")
}

/// [`test`] for a project (see [`check_project`]). A failing test also carries `at` (`path:line:col`).
#[wasm_bindgen]
pub fn test_project(entry: &str, files_json: &str, contracts: bool, dir: &str) -> String {
    let (c, _) = compile_web(entry, files_json, &no_verify());
    let Some(prog) = &c.program else {
        return to_string(&json!({
            "ok": false,
            "report": report_json(&c.report),
            "human": human(&c, &c.report, dir, entry),
        }));
    };
    let results: Vec<Json> = run_tests(prog, &web_options(contracts))
        .into_iter()
        .zip(&prog.tests)
        .map(|(r, t)| match r.outcome {
            Ok(true) => json!({ "name": r.name, "outcome": "pass" }),
            Ok(false) => {
                let (k, local) = c.locate(t.span.start);
                let pos = LineIndex::new(&c.files[k].text).pos(local);
                let shown = if k + 1 == c.files.len() {
                    format!("{dir}{entry}")
                } else {
                    format!("{dir}{}", c.files[k].name)
                };
                json!({ "name": r.name, "outcome": "fail", "at": format!("{shown}:{}:{}", pos.line, pos.col) })
            }
            Err(e) => json!({ "name": r.name, "outcome": "error", "message": e.message, "code": e.code.as_str() }),
        })
        .collect();
    to_string(&json!({ "ok": true, "results": results }))
}

/// Names of the program's zero-argument functions (used by the Studio to know whether a program is a TEA app).
#[wasm_bindgen]
pub fn entry_points(src: &str) -> String {
    let (c, _) = compile_web("main.tez", &single(src), &no_verify());
    let Some(prog) = c.program else {
        return "[]".into();
    };
    let names: Vec<Json> = prog
        .fns
        .iter()
        .filter(|f| !f.is_prelude && f.params.is_empty())
        .map(|f| json!(f.name))
        .collect();
    let _ = Interp::new;
    to_string(&Json::Array(names))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Json {
        serde_json::from_str(s).unwrap_or(Json::Null)
    }

    /// Solves one `query` the way the browser host does: a fresh solver, raw output back.
    fn solve(query: &str) -> Option<String> {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut child = Command::new("z3")
            .args(["-in", "-smt2"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        child.stdin.take()?.write_all(query.as_bytes()).ok()?;
        String::from_utf8(child.wait_with_output().ok()?.stdout).ok()
    }

    /// Drives `verify_step` to completion like the Studio worker does.
    fn verify_in_rounds(src: &str) -> Option<Json> {
        let mut answers = serde_json::Map::new();
        for _ in 0..60 {
            let r = parse(&verify_step(
                src,
                &Json::Object(answers.clone()).to_string(),
            ));
            if r["done"] == true {
                return Some(r);
            }
            for m in r["missing"].as_array()? {
                let raw = solve(m["query"].as_str()?)?;
                answers.insert(m["id"].as_str()?.to_string(), Json::String(raw));
            }
        }
        None
    }

    #[test]
    fn verification_runs_in_rounds_with_an_asynchronous_solver() {
        if solve("(check-sat)").is_none() {
            return; // no z3 on this machine
        }
        let good = verify_in_rounds(
            "fn at(xs: List[Int], i: Int) -> Int\n  requires 0 <= i && i < xs.len()\n  = xs.get(i)\n\nfn main() -> Int = at([1, 2, 3], 1)\n",
        )
        .unwrap();
        assert_eq!(good["verified"], true, "{good}");
        let bad = verify_in_rounds("fn at(xs: List[Int], i: Int) -> Int = xs.get(i)\n").unwrap();
        assert_eq!(bad["verified"], false);
        assert_eq!(bad["report"]["diagnostics"][0]["code"], "E_INDEX_BOUNDS");
        assert!(
            bad["report"]["diagnostics"][0]["counterexample"].is_object(),
            "{bad}"
        );
        // type errors are reported without any solver round
        let typed = parse(&verify_step("fn main() -> Int = \"x\"", "{}"));
        assert_eq!(typed["done"], true);
        assert_eq!(typed["verified"], false);
    }

    #[test]
    fn check_reports_errors_and_never_claims_verification() {
        let ok = parse(&check("fn main() -> Int = 1"));
        assert_eq!(ok["report"]["status"], "ok");
        assert_eq!(ok["verified"], false);
        let bad = parse(&check("fn main() -> Int = \"x\""));
        assert_eq!(bad["report"]["diagnostics"][0]["code"], "E_TYPE_MISMATCH");
    }

    #[test]
    fn run_format_and_test_round_trip() {
        let r = parse(&run("fn main() -> Int = 6 * 7", "main", false));
        assert_eq!(r["ok"], true);
        assert_eq!(r["value"], "42");
        let f = parse(&format("fn   f( a:Int )->Int=a+1"));
        assert_eq!(f["text"], "fn f(a: Int) -> Int = a + 1\n");
        let t = parse(&test("test a = 1 == 1\ntest b = 1 == 2", false));
        assert_eq!(t["results"][0]["outcome"], "pass");
        assert_eq!(t["results"][1]["outcome"], "fail");
    }

    #[test]
    fn build_produces_a_wasm_module() {
        let r = parse(&build("fn main() -> Int = 6 * 7"));
        assert_eq!(r["ok"], true);
        let b64 = r["wasm"].as_str().unwrap_or("");
        assert!(
            b64.starts_with("AGFzbQ"),
            "base64 of the wasm magic number: {b64}"
        );
        assert_eq!(parse(&build("fn main() -> Int = \"x\""))["ok"], false);
    }

    #[test]
    fn runtime_errors_are_diagnostics() {
        let r = parse(&run("fn main() -> Int = 1 / 0", "main", false));
        assert_eq!(r["ok"], false);
        assert_eq!(r["report"]["diagnostics"][0]["code"], "E_DIV_ZERO");
    }

    #[test]
    fn entry_points_lists_zero_argument_functions() {
        let e = parse(&entry_points(
            "fn main() -> Int = 1\nfn f(x: Int) -> Int = x",
        ));
        assert_eq!(e, json!(["main"]));
    }

    #[test]
    fn projects_follow_use_lines_through_the_file_map() {
        let files = json!({
            "main.tez": "use \"lib/math.tez\"\nfn main() -> Int = double(21)\ntest t = double(2) == 4\ntest bad = double(2) == 5\n",
            "lib/math.tez": "fn double(x: Int) -> Int = x * 2\n",
        })
        .to_string();
        let c = parse(&check_project("main.tez", &files, ""));
        assert_eq!(c["report"]["status"], "ok", "{c}");
        let r = parse(&run_project("main.tez", &files, "main", false, ""));
        assert_eq!(r["value"], "42", "{r}");
        let t = parse(&test_project("main.tez", &files, false, "src/"));
        assert_eq!(t["results"][1]["outcome"], "fail");
        assert_eq!(t["results"][1]["at"], "src/main.tez:4:1", "{t}");
        assert_eq!(parse(&build_project("main.tez", &files, ""))["ok"], true);
        // an error inside a dependency is shown with that file's name and text
        let bad = json!({
            "main.tez": "use \"lib/math.tez\"\nfn main() -> Int = double(1)\n",
            "lib/math.tez": "fn double(x: Int) -> Int = \"no\"\n",
        })
        .to_string();
        let b = parse(&check_project("main.tez", &bad, "src/"));
        assert_eq!(b["report"]["diagnostics"][0]["file"], "lib/math.tez");
        assert!(
            b["human"]
                .as_str()
                .unwrap_or("")
                .contains("src/lib/math.tez:1:"),
            "{b}"
        );
        // a missing file is reported, not read from anywhere else
        let missing = parse(&check_project(
            "main.tez",
            "{\"main.tez\": \"use \\\"x.tez\\\"\"}",
            "",
        ));
        assert_eq!(
            missing["report"]["diagnostics"][0]["code"],
            "E_USE_NOT_FOUND"
        );
    }
}
