//! WebAssembly entry points. Every function takes and returns plain strings (JSON), so the
//! JavaScript side stays trivial and the contract is easy to test.
//!
//! Verification needs an SMT solver and is not available in the browser yet; `check` says so
//! explicitly (`"verified": false`) so a UI can never present unproven code as proven.

use serde_json::{Value as Json, json};
use tez_diag::{Diagnostic, Report};
use tez_driver::{CompileOptions, compile};
use tez_fmt::{FmtError, format_source};
use tez_interp::{Interp, Options, run_function, run_tests};
use tez_syntax::{LineIndex, ParseOptions};
use wasm_bindgen::prelude::*;

fn report_json(report: &Report) -> Json {
    serde_json::to_value(report).unwrap_or(Json::Null)
}

fn to_string(v: &Json) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
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
    let c = compile(src, &CompileOptions { verify: false });
    to_string(&json!({ "report": report_json(&c.report), "verified": false }))
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
    let c = compile(src, &CompileOptions { verify: false });
    let Some(prog) = c.program else {
        return to_string(&json!({ "ok": false, "report": report_json(&c.report) }));
    };
    let v = match tez_wasm::compile(&prog) {
        Ok(m) => json!({ "ok": true, "wasm": base64(&m.wasm) }),
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    };
    to_string(&v)
}

/// Runs the zero-argument function `name`. Returns `{ok: true, value}` or `{ok: false, report}`.
#[wasm_bindgen]
pub fn run(src: &str, name: &str, contracts: bool) -> String {
    let c = compile(src, &CompileOptions { verify: false });
    let Some(prog) = c.program else {
        return to_string(&json!({ "ok": false, "report": report_json(&c.report) }));
    };
    let v = match run_function(&prog, name, web_options(contracts)) {
        Ok(value) => json!({ "ok": true, "value": value }),
        Err(e) => {
            let d: Diagnostic = e.to_diagnostic(&LineIndex::new(src));
            json!({ "ok": false, "report": report_json(&Report::new(vec![d])) })
        }
    };
    to_string(&v)
}

/// Runs every `test` item. Returns `{ok, results: [{name, outcome: "pass"|"fail"|"error", message?}]}`.
#[wasm_bindgen]
pub fn test(src: &str, contracts: bool) -> String {
    let c = compile(src, &CompileOptions { verify: false });
    let Some(prog) = c.program else {
        return to_string(&json!({ "ok": false, "report": report_json(&c.report) }));
    };
    let results: Vec<Json> = run_tests(&prog, &web_options(contracts))
        .into_iter()
        .map(|r| match r.outcome {
            Ok(true) => json!({ "name": r.name, "outcome": "pass" }),
            Ok(false) => json!({ "name": r.name, "outcome": "fail" }),
            Err(e) => json!({ "name": r.name, "outcome": "error", "message": e.message, "code": e.code.as_str() }),
        })
        .collect();
    to_string(&json!({ "ok": true, "results": results }))
}

/// Type of the program's `main` value rendered for display, or `null` (used by the Studio to
/// know whether a program is a TEA app). Reserved for the UI runtime.
#[wasm_bindgen]
pub fn entry_points(src: &str) -> String {
    let c = compile(src, &CompileOptions { verify: false });
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
}
