//! The `tez` command-line tool.

use std::process::ExitCode;

use tez_diag::{Diagnostic, Report};
use tez_driver::{CompileOptions, Compiled, compile_project, render_human, render_human_project};
use tez_fmt::{FmtError, format_source};
use tez_interp::{Options, run_function, run_tests, with_big_stack};
use tez_syntax::{ByteSpan, LineIndex, ParseOptions, Problem};

const USAGE: &str = "\
tez: a fast, verified language for LLMs and the browser

USAGE:
    tez check <file> [--json] [--no-verify]  type-check and verify a program
    tez run <file> [fn] [--no-verify] [--contracts]
                                            run a zero-argument function (default: main)
    tez test <file> [--no-verify] [--contracts]
                                            run the `test` items
    tez build <file> [-o out.wasm] [--no-verify]
                                            compile to a WebAssembly (WasmGC) module
    tez fmt <file> [--check | --write]      print / check / rewrite canonical formatting
    tez grammar                             print the normative grammar (EBNF)
    tez --version

Programs are verified before they run. `--no-verify` skips verification (nothing is then
guaranteed); `--contracts` evaluates requires/ensures/refinements while running, which is
how to test a program that is not verified yet.
";

const GRAMMAR: &str = include_str!("../../../spec/grammar.ebnf");

struct Args {
    cmd: String,
    positional: Vec<String>,
    flags: Vec<String>,
    out: Option<String>,
}

fn parse_args() -> Args {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().unwrap_or_default();
    let mut positional = Vec::new();
    let mut flags = Vec::new();
    let mut out = None;
    while let Some(a) = it.next() {
        if a == "-o" {
            out = it.next();
        } else if a.starts_with("--") {
            flags.push(a);
        } else {
            positional.push(a);
        }
    }
    Args {
        cmd,
        positional,
        flags,
        out,
    }
}

impl Args {
    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
}

fn usage_error(msg: &str) -> ExitCode {
    eprintln!("tez: {msg}\n\n{USAGE}");
    ExitCode::from(2)
}

fn read(path: &str) -> Result<String, ExitCode> {
    std::fs::read_to_string(path).map_err(|e| {
        eprintln!("tez: cannot read `{path}`: {e}");
        ExitCode::from(2)
    })
}

fn print_report(report: &Report, src: &str, path: &str, json: bool) {
    if json {
        match serde_json::to_string_pretty(report) {
            Ok(s) => println!("{s}"),
            Err(e) => eprintln!("tez: cannot encode report: {e}"),
        }
    } else {
        eprint!("{}", render_human(report, src, path));
    }
}

fn main() -> ExitCode {
    let args = parse_args();
    match args.cmd.as_str() {
        "--version" | "-V" => {
            println!("tez {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "" | "--help" | "-h" | "help" => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        "grammar" => {
            print!("{GRAMMAR}");
            ExitCode::SUCCESS
        }
        "check" | "run" | "test" | "fmt" | "build" => {
            let Some(path) = args.positional.first().cloned() else {
                return usage_error(&format!("`{}` needs a file", args.cmd));
            };
            let src = match read(&path) {
                Ok(s) => s,
                Err(code) => return code,
            };
            match args.cmd.as_str() {
                "check" => cmd_check(&args, &src, &path),
                "run" => cmd_run(&args, &src, &path),
                "test" => cmd_test(&args, &src, &path),
                "build" => cmd_build(&args, &src, &path),
                _ => cmd_fmt(&args, &src, &path),
            }
        }
        other => usage_error(&format!("unknown command `{other}`")),
    }
}

/// Compiles `path` and the files it `use`s (read from its directory, never from outside it).
fn compile_file(args: &Args, src: &str, path: &str) -> Compiled {
    let p = std::path::Path::new(path);
    let dir = p
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let name = p
        .file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
    let load = |rel: &str| std::fs::read_to_string(dir.join(rel)).ok();
    compile_project(&name, src, &load, &compile_options(args))
}

fn dir_prefix(path: &str) -> String {
    std::path::Path::new(path)
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .map(|d| format!("{}/", d.display()))
        .unwrap_or_default()
}

/// Prints the diagnostics of a compiled program (human text on stderr, or JSON on stdout).
fn print_compiled(c: &Compiled, path: &str, json: bool) {
    if json || c.files.len() <= 1 {
        let src = c.files.last().map_or("", |f| f.text.as_str());
        print_report(&c.report, src, path, json);
    } else {
        eprint!(
            "{}",
            render_human_project(&c.report, &c.files, &dir_prefix(path), path)
        );
    }
}

/// A runtime failure as a diagnostic positioned in the file it happened in.
fn runtime_diagnostic(c: &Compiled, e: &tez_interp::RuntimeError) -> Diagnostic {
    let (k, local) = c.locate(e.span.start);
    let file = &c.files[k];
    let len = e.span.end.saturating_sub(e.span.start);
    let mut d = Problem::new(e.code, e.message.clone(), ByteSpan::new(local, local + len))
        .to_diagnostic(&LineIndex::new(&file.text));
    d.phase = tez_diag::Phase::Runtime;
    if c.files.len() > 1 {
        d.file = Some(file.name.clone());
    }
    d
}

fn cmd_check(args: &Args, src: &str, path: &str) -> ExitCode {
    let c = compile_file(args, src, path);
    let json = args.flag("--json");
    if c.report.diagnostics.is_empty() {
        if json {
            print_compiled(&c, path, true);
        } else {
            if args.flag("--no-verify") {
                println!("ok: {path} (NOT verified: --no-verify)");
            } else {
                println!("ok: {path}");
            }
        }
        ExitCode::SUCCESS
    } else {
        print_compiled(&c, path, json);
        ExitCode::from(1)
    }
}

fn compile_options(args: &Args) -> CompileOptions {
    CompileOptions {
        verify: !args.flag("--no-verify"),
        ..CompileOptions::default()
    }
}

fn interp_options(args: &Args) -> Options {
    Options {
        check_contracts: args.flag("--contracts"),
        ..Options::default()
    }
}

fn cmd_run(args: &Args, src: &str, path: &str) -> ExitCode {
    let c = compile_file(args, src, path);
    let Some(prog) = &c.program else {
        print_compiled(&c, path, args.flag("--json"));
        return ExitCode::from(1);
    };
    let name = args.positional.get(1).map_or("main", String::as_str);
    let opts = interp_options(args);
    match with_big_stack(|| run_function(prog, name, opts)) {
        Ok(v) => {
            println!("{v}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            let d = runtime_diagnostic(&c, &e);
            let one = Compiled {
                report: Report::new(vec![d]),
                program: None,
                files: c.files.clone(),
                merged: String::new(),
                ranges: Vec::new(),
            };
            print_compiled(&one, path, args.flag("--json"));
            ExitCode::from(1)
        }
    }
}

fn cmd_build(args: &Args, src: &str, path: &str) -> ExitCode {
    let c = compile_file(args, src, path);
    let Some(prog) = &c.program else {
        print_compiled(&c, path, args.flag("--json"));
        return ExitCode::from(1);
    };
    let module = match tez_wasm::compile(prog) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("tez: code generation failed: {e}");
            return ExitCode::from(1);
        }
    };
    let out = args
        .out
        .clone()
        .unwrap_or_else(|| format!("{}.wasm", path.trim_end_matches(".tez")));
    if let Err(e) = std::fs::write(&out, &module.wasm) {
        eprintln!("tez: cannot write `{out}`: {e}");
        return ExitCode::from(2);
    }
    println!(
        "built {out} ({} bytes); run it with `node runtime/run.mjs {out}`",
        module.wasm.len()
    );
    ExitCode::SUCCESS
}

fn cmd_test(args: &Args, src: &str, path: &str) -> ExitCode {
    let c = compile_file(args, src, path);
    let Some(prog) = &c.program else {
        print_compiled(&c, path, args.flag("--json"));
        return ExitCode::from(1);
    };
    let opts = interp_options(args);
    let results = with_big_stack(|| run_tests(prog, &opts));
    let mut failed = 0;
    for (r, t) in results.iter().zip(&prog.tests) {
        match &r.outcome {
            Ok(true) => println!("ok      {}", r.name),
            Ok(false) => {
                failed += 1;
                let (k, local) = c.locate(t.span.start);
                let pos = LineIndex::new(&c.files[k].text).pos(local);
                let shown = if k + 1 == c.files.len() {
                    path.to_string()
                } else {
                    format!("{}{}", dir_prefix(path), c.files[k].name)
                };
                println!("FAILED  {}  ({shown}:{}:{})", r.name, pos.line, pos.col);
            }
            Err(e) => {
                failed += 1;
                println!("ERROR   {}: {} [{}]", r.name, e.message, e.code.as_str());
            }
        }
    }
    println!("{} passed, {failed} failed", results.len() - failed);
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn cmd_fmt(args: &Args, src: &str, path: &str) -> ExitCode {
    match format_source(src, ParseOptions::default()) {
        Ok(out) => {
            if args.flag("--check") {
                if out == src {
                    return ExitCode::SUCCESS;
                }
                eprintln!("tez: {path} is not formatted");
                return ExitCode::from(1);
            }
            if args.flag("--write") {
                if out != src
                    && let Err(e) = std::fs::write(path, &out)
                {
                    eprintln!("tez: cannot write `{path}`: {e}");
                    return ExitCode::from(2);
                }
                return ExitCode::SUCCESS;
            }
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(FmtError::Syntax(problems)) => {
            let index = LineIndex::new(src);
            let report = Report::new(problems.iter().map(|p| p.to_diagnostic(&index)).collect());
            print_report(&report, src, path, false);
            ExitCode::from(1)
        }
        Err(FmtError::Comment(span)) => {
            let pos = LineIndex::new(src).pos(span.start);
            eprintln!(
                "tez: {path}:{}:{}: a comment sits inside an expression; move it onto its own line before a statement",
                pos.line, pos.col
            );
            ExitCode::from(1)
        }
    }
}
