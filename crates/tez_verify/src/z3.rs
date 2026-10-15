//! A Z3 process driven over SMT-LIB 2 (native builds only).

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

use sha2::{Digest, Sha256};

/// How many `check-sat` answers came from the proof cache instead of the solver (diagnostics and tests).
pub static CACHE_HITS: AtomicUsize = AtomicUsize::new(0);

/// Where proofs are cached: `$TEZ_CACHE_DIR`, else `~/.cache/tez/vc`; `TEZ_NO_CACHE` turns the cache off.
#[must_use]
pub fn default_cache_dir() -> Option<PathBuf> {
    if std::env::var_os("TEZ_NO_CACHE").is_some() {
        return None;
    }
    if let Some(d) = std::env::var_os("TEZ_CACHE_DIR") {
        return Some(PathBuf::from(d));
    }
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/tez/vc"))
}

/// Answer to `(check-sat)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// The negated goal is unsatisfiable: the obligation holds.
    Proved,
    /// A model exists: the obligation can be violated.
    Refuted,
    /// The solver gave up (timeout, incompleteness).
    Unknown,
}

/// A running `z3 -in -smt2` process (native builds).
pub struct ProcessZ3 {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    /// Everything sent so far (and the solver's version): a proved script is remembered under its hash.
    /// Only `unsat` answers are cached, so a stale or foreign entry can never turn a refutation into a proof
    /// unless the *identical* script with the *same solver* was proved before.
    script: Sha256,
    cache: Option<PathBuf>,
}

impl ProcessZ3 {
    /// Starts Z3 with a per-query timeout.
    ///
    /// # Errors
    /// Returns a message when the `z3` executable cannot be started.
    #[cfg(test)]
    pub fn spawn(timeout_ms: u32) -> Result<ProcessZ3, String> {
        Self::spawn_cached(timeout_ms, None)
    }

    /// Starts Z3 remembering proved queries in `cache` (a directory).
    ///
    /// # Errors
    /// Returns a message when the `z3` executable cannot be started.
    pub fn spawn_cached(timeout_ms: u32, cache: Option<PathBuf>) -> Result<ProcessZ3, String> {
        let mut child = Command::new("z3")
            .args(["-in", "-smt2"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start `z3`: {e}"))?;
        let stdin = child.stdin.take().ok_or("z3 has no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("z3 has no stdout")?);
        let mut z = ProcessZ3 {
            child,
            stdin,
            stdout,
            script: Sha256::new(),
            cache,
        };
        z.stdin
            .write_all(b"(get-info :version)\n")
            .and_then(|()| z.stdin.flush())
            .map_err(|e| format!("z3 write error: {e}"))?;
        let version = z.line()?;
        z.script.update(version.as_bytes());
        z.run(&[
            "(set-option :print-success true)".to_string(),
            format!("(set-option :timeout {timeout_ms})"),
        ])?;
        Ok(z)
    }

    fn line(&mut self) -> Result<String, String> {
        let mut s = String::new();
        let n = self
            .stdout
            .read_line(&mut s)
            .map_err(|e| format!("z3 read error: {e}"))?;
        if n == 0 {
            return Err("z3 closed its output".into());
        }
        Ok(s.trim().to_string())
    }

    /// Sends commands that answer `success`, in batches.
    ///
    /// # Errors
    /// Returns the solver's complaint if any command is rejected.
    pub fn run(&mut self, cmds: &[String]) -> Result<(), String> {
        for chunk in cmds.chunks(200) {
            let mut text = String::new();
            for c in chunk {
                text.push_str(c);
                text.push('\n');
            }
            self.script.update(text.as_bytes());
            self.stdin
                .write_all(text.as_bytes())
                .map_err(|e| format!("z3 write error: {e}"))?;
            self.stdin
                .flush()
                .map_err(|e| format!("z3 write error: {e}"))?;
            let mut first_error: Option<String> = None;
            for c in chunk {
                let l = self.line()?;
                if l != "success" && first_error.is_none() {
                    first_error = Some(format!(
                        "{l}  [in: {}]",
                        c.chars().take(200).collect::<String>()
                    ));
                }
            }
            if let Some(e) = first_error {
                return Err(e);
            }
        }
        Ok(())
    }

    /// `(check-sat)`.
    ///
    /// # Errors
    /// Returns a message when the solver misbehaves.
    pub fn check_sat(&mut self) -> Result<Verdict, String> {
        let key = self.cache.as_ref().map(|dir| {
            let digest = self.script.clone().finalize();
            let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
            dir.join(hex)
        });
        if let Some(path) = &key
            && path.is_file()
        {
            CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            return Ok(Verdict::Proved);
        }
        let verdict = self.check_sat_uncached()?;
        if verdict == Verdict::Proved
            && let Some(path) = &key
        {
            // best effort: a read-only or missing cache directory only costs speed
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, b"");
        }
        Ok(verdict)
    }

    fn check_sat_uncached(&mut self) -> Result<Verdict, String> {
        self.stdin
            .write_all(b"(check-sat)\n")
            .map_err(|e| format!("z3 write error: {e}"))?;
        self.stdin
            .flush()
            .map_err(|e| format!("z3 write error: {e}"))?;
        let l = self.line()?;
        // print-success also acknowledges the command itself only for non-query commands
        match l.as_str() {
            "unsat" => Ok(Verdict::Proved),
            "sat" => Ok(Verdict::Refuted),
            "unknown" | "timeout" => Ok(Verdict::Unknown),
            other => Err(format!("unexpected z3 answer: {other}")),
        }
    }

    /// `(get-value (t1 t2 ..))` after a `sat` answer; returns one rendered value per term.
    ///
    /// # Errors
    /// Returns a message when the solver misbehaves.
    pub fn get_values(&mut self, terms: &[String]) -> Result<Vec<String>, String> {
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let cmd = format!("(get-value ({}))\n", terms.join(" "));
        self.stdin
            .write_all(cmd.as_bytes())
            .map_err(|e| format!("z3 write error: {e}"))?;
        self.stdin
            .flush()
            .map_err(|e| format!("z3 write error: {e}"))?;
        // read until parentheses balance
        let mut text = String::new();
        let mut depth: i64 = 0;
        loop {
            let l = self.line()?;
            for c in l.chars() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
            }
            text.push_str(&l);
            text.push(' ');
            if depth <= 0 {
                break;
            }
        }
        if text.trim_start().starts_with("(error") {
            return Err(text);
        }
        Ok(parse_values(&text, terms.len()))
    }
}

impl Drop for ProcessZ3 {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"(exit)\n");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Splits `((t1 v1) (t2 v2) ..)` into the rendered values.
fn parse_values(text: &str, n: usize) -> Vec<String> {
    let chars: Vec<char> = text.trim().chars().collect();
    let mut out = Vec::new();
    let mut i = 1; // skip the outer `(`
    while i < chars.len() && out.len() < n {
        if chars[i] != '(' {
            i += 1;
            continue;
        }
        // one `(term value)` pair
        let mut depth = 0;
        let start = i;
        let mut end = i;
        while end < chars.len() {
            match chars[end] {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            end += 1;
        }
        let pair: String = chars[start + 1..end].iter().collect();
        // the value starts after the term, which is itself an s-expression or a symbol
        let value = split_after_first_sexp(&pair);
        out.push(pretty_value(value.trim()));
        i = end + 1;
    }
    out
}

fn split_after_first_sexp(s: &str) -> &str {
    let b = s.trim_start();
    let mut depth = 0;
    for (i, c) in b.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if c.is_whitespace() && depth == 0 => return &b[i..],
            _ => {}
        }
    }
    ""
}

/// `(- 3)` becomes `-3`; everything else is returned as-is.
fn pretty_value(v: &str) -> String {
    let t = v.trim();
    if let Some(rest) = t.strip_prefix("(-").and_then(|r| r.strip_suffix(')')) {
        let n = rest.trim();
        if n.chars().all(|c| c.is_ascii_digit()) && !n.is_empty() {
            return format!("-{n}");
        }
    }
    t.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_get_value_output() {
        let v = parse_values("((a 5) (b (- 3)) ((len_L xs) 2) (s (Circle (- 1))))", 4);
        assert_eq!(v, vec!["5", "-3", "2", "(Circle (- 1))"]);
    }

    #[test]
    fn z3_round_trip_if_installed() {
        let Ok(mut z) = ProcessZ3::spawn(2000) else {
            return;
        };
        z.run(&["(declare-const x Int)".into(), "(assert (> x 3))".into()])
            .unwrap();
        assert_eq!(z.check_sat().unwrap(), Verdict::Refuted);
        assert_eq!(z.get_values(&["x".into()]).unwrap().len(), 1);
        z.run(&["(assert (< x 2))".into()]).unwrap();
        assert_eq!(z.check_sat().unwrap(), Verdict::Proved);
    }
}

// ------------------------------------------------------------------ replay backend

/// A query the verifier needed and had no answer for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    /// Hash of `query`; the host answers under this key.
    pub id: String,
    /// The complete script to evaluate in a fresh solver (starts with `(reset)`), ending in `(check-sat)` and,
    /// for model queries, `(get-value ..)`. The raw solver output is the answer.
    pub query: String,
    /// True when the answer must also contain the requested values.
    pub wants_values: bool,
}

thread_local! {
    static ANSWERS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static MISSING: RefCell<Vec<Missing>> = const { RefCell::new(Vec::new()) };
}

/// Installs the answers known so far (id -> raw solver output) and forgets earlier missing queries.
pub fn replay_set_answers(answers: HashMap<String, String>) {
    ANSWERS.with(|a| *a.borrow_mut() = answers);
    MISSING.with(|m| m.borrow_mut().clear());
}

/// The queries the last verification run needed and did not have.
#[must_use]
pub fn replay_take_missing() -> Vec<Missing> {
    MISSING.with(|m| std::mem::take(&mut *m.borrow_mut()))
}

fn hex_digest(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The solver as a script recorder (see the module docs).
pub struct ReplayZ3 {
    script: String,
}

impl ReplayZ3 {
    /// Starts an empty session.
    #[must_use]
    pub fn new() -> ReplayZ3 {
        ReplayZ3 {
            script: String::from("(reset)\n"),
        }
    }

    fn lookup(&self, query: String, wants_values: bool) -> Result<Option<String>, String> {
        let id = hex_digest(&query);
        if let Some(raw) = ANSWERS.with(|a| a.borrow().get(&id).cloned()) {
            return Ok(Some(raw));
        }
        MISSING.with(|m| {
            let mut m = m.borrow_mut();
            if !m.iter().any(|x| x.id == id) {
                m.push(Missing {
                    id,
                    query,
                    wants_values,
                });
            }
        });
        Ok(None)
    }

    /// Records commands (nothing is sent anywhere).
    ///
    /// # Errors
    /// Never.
    pub fn run(&mut self, cmds: &[String]) -> Result<(), String> {
        for c in cmds {
            self.script.push_str(c);
            self.script.push('\n');
        }
        Ok(())
    }

    /// `(check-sat)` answered from the table, or `Unknown` plus a recorded request.
    ///
    /// # Errors
    /// When the host reported a solver error for this script.
    pub fn check_sat(&mut self) -> Result<Verdict, String> {
        let query = format!("{}(check-sat)\n", self.script);
        let Some(raw) = self.lookup(query, false)? else {
            return Ok(Verdict::Unknown);
        };
        verdict_of(&raw)
    }

    /// Values after a `sat` answer, from the table; placeholders while the request is pending.
    ///
    /// # Errors
    /// When the host reported a solver error for this script.
    pub fn get_values(&mut self, terms: &[String]) -> Result<Vec<String>, String> {
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        let query = format!(
            "{}(check-sat)\n(get-value ({}))\n",
            self.script,
            terms.join(" ")
        );
        let Some(raw) = self.lookup(query, true)? else {
            return Ok(vec!["?".to_string(); terms.len()]);
        };
        let after = raw
            .lines()
            .map(str::trim)
            .skip_while(|l| !matches!(*l, "sat" | "unsat" | "unknown" | "timeout"))
            .skip(1)
            .collect::<Vec<_>>()
            .join(" ");
        if after.trim_start().starts_with("(error") {
            return Err(after);
        }
        Ok(parse_values(&after, terms.len()))
    }
}

impl Default for ReplayZ3 {
    fn default() -> Self {
        Self::new()
    }
}

/// Reads a `check-sat` answer out of raw solver output (which may also contain `success` acknowledgements).
fn verdict_of(raw: &str) -> Result<Verdict, String> {
    for l in raw.lines().map(str::trim) {
        match l {
            "unsat" => return Ok(Verdict::Proved),
            "sat" => return Ok(Verdict::Refuted),
            "unknown" | "timeout" => return Ok(Verdict::Unknown),
            "" | "success" => {}
            other if other.starts_with("(error") => return Err(other.to_string()),
            _ => {}
        }
    }
    Err(format!(
        "no verdict in solver output: {}",
        raw.chars().take(200).collect::<String>()
    ))
}

// ------------------------------------------------------------------ the one type the verifier uses

/// Which solver backend to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    /// A native `z3` child process.
    Process,
    /// Record scripts and take answers from the host (see [`replay_set_answers`]).
    Replay,
}

impl Default for Backend {
    fn default() -> Self {
        if cfg!(target_arch = "wasm32") {
            Backend::Replay
        } else {
            Backend::Process
        }
    }
}

/// A solver session.
pub enum Z3 {
    /// See [`ProcessZ3`].
    Process(ProcessZ3),
    /// See [`ReplayZ3`].
    Replay(ReplayZ3),
}

impl Z3 {
    /// Opens a session on the chosen backend.
    ///
    /// # Errors
    /// When the process backend cannot start `z3`.
    pub fn open(backend: Backend, timeout_ms: u32, cache: Option<PathBuf>) -> Result<Z3, String> {
        match backend {
            Backend::Process => Ok(Z3::Process(ProcessZ3::spawn_cached(timeout_ms, cache)?)),
            Backend::Replay => {
                let mut z = ReplayZ3::new();
                z.run(&[
                    "(set-option :print-success true)".to_string(),
                    format!("(set-option :timeout {timeout_ms})"),
                ])?;
                Ok(Z3::Replay(z))
            }
        }
    }

    /// Sends commands.
    ///
    /// # Errors
    /// The solver's complaint.
    pub fn run(&mut self, cmds: &[String]) -> Result<(), String> {
        match self {
            Z3::Process(z) => z.run(cmds),
            Z3::Replay(z) => z.run(cmds),
        }
    }

    /// `(check-sat)`.
    ///
    /// # Errors
    /// When the solver misbehaves.
    pub fn check_sat(&mut self) -> Result<Verdict, String> {
        match self {
            Z3::Process(z) => z.check_sat(),
            Z3::Replay(z) => z.check_sat(),
        }
    }

    /// `(get-value ..)` after `sat`.
    ///
    /// # Errors
    /// When the solver misbehaves.
    pub fn get_values(&mut self, terms: &[String]) -> Result<Vec<String>, String> {
        match self {
            Z3::Process(z) => z.get_values(terms),
            Z3::Replay(z) => z.get_values(terms),
        }
    }
}
