//! Writes /tmp/perf.wasm for manual benchmarking (ignored by default).
#![allow(clippy::panic, clippy::unwrap_used, clippy::expect_used)]
use tez_syntax::{ParseOptions, parse};
use tez_types::check;

#[test]
#[ignore]
fn build_perf() {
    let src = std::fs::read_to_string("/tmp/perf.tez").unwrap();
    let parsed = parse(&src, ParseOptions::default());
    let out = check(&parsed.file, &src);
    assert!(out.problems.is_empty(), "{:?}", out.problems);
    std::fs::write(
        "/tmp/perf.wasm",
        tez_wasm::compile(&out.program).unwrap().wasm,
    )
    .unwrap();
}
