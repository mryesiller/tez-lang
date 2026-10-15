//! Robustness: no input may crash the front end, and formatting is idempotent.
//!
//! Deterministic mutation fuzzing over the example programs (a seeded generator, so failures reproduce).
//! `TEZ_FUZZ_N` sets the mutations per file (default 300).

#![allow(clippy::unwrap_used)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use tez_driver::{CompileOptions, compile};
use tez_fmt::format_source;
use tez_syntax::{ParseOptions, parse};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n.max(1) as u64)).unwrap_or(0)
    }
}

const PIECES: [&str; 28] = [
    "fn",
    "type",
    "match",
    "if",
    "else",
    "let",
    "where",
    "with",
    "requires",
    "ensures",
    "decreases",
    "=>",
    "->",
    "{",
    "}",
    "(",
    ")",
    "[",
    "]",
    "..",
    "|",
    "&&",
    "||",
    "++",
    "\"",
    "//",
    "é",
    "9999999999999999999999",
];

fn mutate(src: &str, rng: &mut Rng) -> String {
    let chars: Vec<char> = src.chars().collect();
    if chars.is_empty() {
        return String::new();
    }
    let mut out = chars.clone();
    for _ in 0..=rng.below(3) {
        let i = rng.below(out.len().max(1));
        match rng.below(6) {
            0 if !out.is_empty() => {
                let j = (i + 1 + rng.below(12)).min(out.len());
                out.drain(i.min(out.len())..j);
            }
            1 => {
                let piece: Vec<char> = PIECES[rng.below(PIECES.len())].chars().collect();
                let at = i.min(out.len());
                out.splice(at..at, piece);
            }
            2 if !out.is_empty() => {
                let j = (i + 1 + rng.below(30)).min(out.len());
                let dup: Vec<char> = out[i.min(out.len())..j].to_vec();
                let at = rng.below(out.len() + 1);
                out.splice(at..at, dup);
            }
            3 => out.truncate(i),
            4 if !out.is_empty() => {
                let a = i.min(out.len() - 1);
                let b = rng.below(out.len());
                out.swap(a, b);
            }
            _ => {
                let at = i.min(out.len());
                out.insert(
                    at,
                    ['\n', ' ', '\t', '\u{0}', 'ß', '😀', '\r'][rng.below(7)],
                );
            }
        }
    }
    out.into_iter().collect()
}

fn corpus() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/examples");
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "tez"))
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect();
    files.sort();
    files
}

#[test]
fn mutated_programs_never_crash_the_front_end() {
    let n: usize = std::env::var("TEZ_FUZZ_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut parsed_ok, mut formatted) = (0usize, 0usize);
    for (name, src) in corpus() {
        for round in 0..n {
            let m = mutate(&src, &mut rng);
            let r = catch_unwind(AssertUnwindSafe(|| {
                // full front end without the solver
                let _ = compile(
                    &m,
                    &CompileOptions {
                        verify: false,
                        ..CompileOptions::default()
                    },
                );
                let ok = parse(&m, ParseOptions::default()).problems.is_empty();
                if ok && let Ok(once) = format_source(&m, ParseOptions::default()) {
                    {
                        let twice =
                            format_source(&once, ParseOptions::default()).unwrap_or_else(|e| {
                                panic!("formatted output no longer formats: {e:?}\n{once}")
                            });
                        assert_eq!(once, twice, "formatting is not idempotent");
                        assert!(
                            parse(&once, ParseOptions::default()).problems.is_empty(),
                            "formatted output does not parse:\n{once}"
                        );
                        return (true, true);
                    }
                }
                (ok, false)
            }));
            match r {
                Ok((p, f)) => {
                    parsed_ok += usize::from(p);
                    formatted += usize::from(f);
                }
                Err(_) => {
                    panic!("front end crashed or broke a property on {name} (round {round}):\n{m}")
                }
            }
        }
    }
    assert!(
        parsed_ok > 0 && formatted > 0,
        "the fuzzer produced no valid programs to format"
    );
}
