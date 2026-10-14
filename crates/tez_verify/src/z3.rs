//! A Z3 process driven over SMT-LIB 2 (native builds only).

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

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

/// A running `z3 -in -smt2` process.
pub struct Z3 {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Z3 {
    /// Starts Z3 with a per-query timeout.
    ///
    /// # Errors
    /// Returns a message when the `z3` executable cannot be started.
    pub fn spawn(timeout_ms: u32) -> Result<Z3, String> {
        let mut child = Command::new("z3")
            .args(["-in", "-smt2"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("cannot start `z3`: {e}"))?;
        let stdin = child.stdin.take().ok_or("z3 has no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("z3 has no stdout")?);
        let mut z = Z3 {
            child,
            stdin,
            stdout,
        };
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

impl Drop for Z3 {
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
        let Ok(mut z) = Z3::spawn(2000) else { return };
        z.run(&["(declare-const x Int)".into(), "(assert (> x 3))".into()])
            .unwrap();
        assert_eq!(z.check_sat().unwrap(), Verdict::Refuted);
        assert_eq!(z.get_values(&["x".into()]).unwrap().len(), 1);
        z.run(&["(assert (< x 2))".into()]).unwrap();
        assert_eq!(z.check_sat().unwrap(), Verdict::Proved);
    }
}
