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
    tez new <dir> [--app]                    start a project: a small program, or a web app
    tez check <file> [--json] [--no-verify]  type-check and verify a program
    tez run <file> [fn] [--no-verify] [--contracts]
                                            run a zero-argument function (default: main)
    tez test <file> [--no-verify] [--contracts]
                                            run the `test` items
    tez build <file> [-o out.wasm] [--no-verify]
                                            compile to a WebAssembly (WasmGC) module
    tez fmt <file|dir> [--check | --write]  print / check / rewrite canonical formatting
                                            (a directory: every .tez file below it; needs --check or --write)
    tez grammar                             print the normative grammar (EBNF)
    tez --version

`check`, `run`, `test` and `build` take `--watch`: run again whenever a .tez file next to the entry file
(or below it) changes; stop with Ctrl-C.

Programs are verified before they run. `--no-verify` skips verification (nothing is then
guaranteed); `--contracts` evaluates requires/ensures/refinements while running, which is
how to test a program that is not verified yet.
";

/// The flags each command accepts; anything else is a usage error (a typo must not be ignored silently).
const FLAGS: &[(&str, &[&str])] = &[
    ("check", &["--json", "--no-verify", "--watch"]),
    ("run", &["--json", "--no-verify", "--contracts", "--watch"]),
    ("test", &["--json", "--no-verify", "--contracts", "--watch"]),
    ("build", &["--json", "--no-verify", "--watch"]),
    ("fmt", &["--check", "--write"]),
    ("new", &["--app"]),
];

const TEMPLATE_PROGRAM: &str = include_str!("../../../spec/templates/program.tez");
const TEMPLATE_APP: &str = include_str!("../../../spec/templates/app.tez");

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
        } else if a == "-h" {
            flags.push("--help".to_string());
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
    // `tez --help`, `tez help` and `tez <command> --help` print the usage and succeed; `tez` alone is a usage error
    if args.flag("--help") || matches!(args.cmd.as_str(), "--help" | "help") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if let Some((_, allowed)) = FLAGS.iter().find(|(c, _)| *c == args.cmd)
        && let Some(bad) = args.flags.iter().find(|f| !allowed.contains(&f.as_str()))
    {
        return usage_error(&format!("`{}` does not take `{bad}`", args.cmd));
    }
    match args.cmd.as_str() {
        "--version" | "-V" => {
            println!("tez {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "" => usage_error("a command is needed"),
        "grammar" => {
            print!("{GRAMMAR}");
            ExitCode::SUCCESS
        }
        "new" => cmd_new(&args),
        "check" | "run" | "test" | "fmt" | "build" => {
            let Some(path) = args.positional.first().cloned() else {
                return usage_error(&format!("`{}` needs a file", args.cmd));
            };
            if args.cmd == "fmt" && std::path::Path::new(&path).is_dir() {
                return cmd_fmt_dir(&args, &path);
            }
            if args.flag("--watch") {
                return watch(&args, &path);
            }
            run_command(&args, &path)
        }
        other => usage_error(&format!("unknown command `{other}`")),
    }
}

/// Reads `path` and runs the command once.
fn run_command(args: &Args, path: &str) -> ExitCode {
    let src = match read(path) {
        Ok(s) => s,
        Err(code) => return code,
    };
    match args.cmd.as_str() {
        "check" => cmd_check(args, &src, path),
        "run" => cmd_run(args, &src, path),
        "test" => cmd_test(args, &src, path),
        "build" => cmd_build(args, &src, path),
        _ => cmd_fmt(args, &src, path),
    }
}

/// Every `.tez` file next to the entry file or below it, with its modification time and size.
fn tez_files(entry: &str) -> Vec<(std::path::PathBuf, Option<std::time::SystemTime>, u64)> {
    fn walk(
        dir: &std::path::Path,
        depth: usize,
        out: &mut Vec<(std::path::PathBuf, Option<std::time::SystemTime>, u64)>,
    ) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                if depth < 6 && !name.starts_with('.') && name != "target" && name != "node_modules"
                {
                    walk(&p, depth + 1, out);
                }
            } else if p.extension().is_some_and(|x| x == "tez") {
                out.push((p, meta.modified().ok(), meta.len()));
            }
        }
    }
    let dir = std::path::Path::new(entry)
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let mut out = Vec::new();
    walk(dir, 0, &mut out);
    out.sort();
    out
}

/// Runs the command, then again every time a `.tez` file of the project changes. Never returns.
fn watch(args: &Args, path: &str) -> ExitCode {
    use std::io::{IsTerminal, Write};
    let tty = std::io::stdout().is_terminal();
    let mut seen = tez_files(path);
    loop {
        if tty {
            print!("\x1b[2J\x1b[H");
        }
        run_command(args, path);
        println!("\n[watching {path}: it runs again when a .tez file changes; Ctrl-C stops]");
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(250));
            if tez_files(path) != seen {
                // an editor may write in several steps: wait until the files hold still
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    let now = tez_files(path);
                    if now == tez_files(path) {
                        seen = now;
                        break;
                    }
                }
                break;
            }
        }
    }
}

/// `tez new <dir> [--app]`: a directory with a `main.tez` that already checks, tests and runs.
fn cmd_new(args: &Args) -> ExitCode {
    let Some(dir) = args.positional.first() else {
        return usage_error("`new` needs a directory name");
    };
    let target = std::path::Path::new(dir);
    if target.exists() && !target.is_dir() {
        eprintln!("tez: `{dir}` exists and is not a directory");
        return ExitCode::from(2);
    }
    if std::fs::read_dir(target).is_ok_and(|mut d| d.next().is_some()) {
        eprintln!("tez: `{dir}` is not empty; nothing was written");
        return ExitCode::from(2);
    }
    let app = args.flag("--app");
    let text = if app { TEMPLATE_APP } else { TEMPLATE_PROGRAM };
    let main = target.join("main.tez");
    if let Err(e) = std::fs::create_dir_all(target).and_then(|()| std::fs::write(&main, text)) {
        eprintln!("tez: cannot write `{}`: {e}", main.display());
        return ExitCode::from(2);
    }
    println!("created {}", main.display());
    println!("next:  cd {dir} && tez check main.tez && tez test main.tez");
    if app {
        println!(
            "       open main.tez in the Studio (https://mryesiller.github.io/tez-lang/studio/) to run the app"
        );
    } else {
        println!("       tez run main.tez");
    }
    ExitCode::SUCCESS
}

/// `tez fmt <dir> --check|--write`: every `.tez` file below the directory.
fn cmd_fmt_dir(args: &Args, dir: &str) -> ExitCode {
    if !args.flag("--check") && !args.flag("--write") {
        return usage_error("`fmt` on a directory needs --check or --write");
    }
    let files = tez_files(&format!("{}/x", dir.trim_end_matches('/')));
    if files.is_empty() {
        eprintln!("tez: no .tez files below `{dir}`");
        return ExitCode::from(2);
    }
    let mut worst = 0;
    for (p, _, _) in files {
        let path = p.to_string_lossy().into_owned();
        let src = match read(&path) {
            Ok(s) => s,
            Err(code) => return code,
        };
        let code = cmd_fmt(args, &src, &path);
        if code != ExitCode::SUCCESS {
            worst = worst.max(if code == ExitCode::from(2) { 2 } else { 1 });
        }
    }
    ExitCode::from(worst)
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
