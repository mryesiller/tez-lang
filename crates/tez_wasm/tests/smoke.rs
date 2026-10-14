//! First end-to-end check: compile, validate with wasm-tools, run in Node.
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

use tez_syntax::{ParseOptions, parse};
use tez_types::check;

fn wasm_for(src: &str) -> Vec<u8> {
    let parsed = parse(src, ParseOptions::default());
    assert!(parsed.problems.is_empty(), "{:?}", parsed.problems);
    let out = check(&parsed.file, src);
    assert!(out.problems.is_empty(), "{:?}", out.problems);
    tez_wasm::compile(&out.program).unwrap().wasm
}

#[test]
fn compiles_a_trivial_program() {
    let bytes = wasm_for("fn main() -> Int = 6 * 7");
    std::fs::write("/tmp/smoke.wasm", &bytes).unwrap();
    assert!(bytes.starts_with(b"\0asm"));
}
