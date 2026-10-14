//! The `tez` command-line tool.
//!
//! Subcommands are added phase by phase. Anything not implemented yet fails loudly:
//! a stub that reported "ok" would be unsound.

use std::process::ExitCode;

const USAGE: &str = "\
tez — fast, verified, LLM-friendly language for the browser

USAGE:
    tez --version
    tez check <file> [--json]    (Phase 1, not implemented)
    tez fmt <file>               (Phase 1, not implemented)
    tez run <file>               (Phase 1, not implemented)
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("tez {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help" | "-h") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(cmd @ ("check" | "fmt" | "run")) => {
            eprintln!("tez {cmd}: not implemented yet (see docs/ROADMAP.md, Phase 1)");
            ExitCode::from(2)
        }
        Some(other) => {
            eprintln!("tez: unknown command `{other}`\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}
