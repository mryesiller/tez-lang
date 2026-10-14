//! Reference interpreter for checked Tez programs.
//!
//! It is the semantic oracle: the WebAssembly back end must agree with it on every program.

mod interp;
mod native;
mod value;

use tez_diag::{Code, Diagnostic};
use tez_syntax::LineIndex;
use tez_types::hir::{FnId, Program};

pub use interp::{Interp, Options, RuntimeError};
pub use value::{ListVal, Value, fmt_float, values_eq};

impl RuntimeError {
    /// Converts to a diagnostic (phase `runtime`).
    #[must_use]
    pub fn to_diagnostic(&self, index: &LineIndex<'_>) -> Diagnostic {
        Diagnostic::new(self.code, self.message.clone(), index.span(self.span))
            .in_phase(tez_diag::Phase::Runtime)
    }
}

/// Outcome of one `test` item.
#[derive(Debug)]
pub struct TestResult {
    /// Test name.
    pub name: String,
    /// `Ok(true)` passed, `Ok(false)` failed, `Err` crashed.
    pub outcome: Result<bool, RuntimeError>,
}

/// Runs `f` on a thread with a large stack so deep Tez recursion cannot overflow Rust's stack.
pub fn with_big_stack<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(1 << 30)
            .spawn_scoped(s, f)
            .map_or_else(
                |_| panic_free_fallback(),
                |h| h.join().unwrap_or_else(|_| panic_free_fallback()),
            )
    })
}

#[allow(clippy::panic)]
fn panic_free_fallback<T>() -> T {
    panic!("interpreter thread failed")
}

/// Evaluates the zero-argument function `name` and renders its value.
///
/// # Errors
/// Returns the run-time failure, or `E_INTERNAL` if the function is missing or takes arguments.
pub fn run_function(prog: &Program, name: &str, opts: Options) -> Result<String, RuntimeError> {
    let id = find_nullary(prog, name)?;
    let mut it = Interp::new(prog, opts);
    let v = it.call(id, Vec::new())?;
    Ok(v.display(prog))
}

fn find_nullary(prog: &Program, name: &str) -> Result<FnId, RuntimeError> {
    let err = |m: String| RuntimeError {
        code: Code::Internal,
        message: m,
        span: tez_syntax::ByteSpan::new(0, 0),
    };
    let id = prog
        .find_fn(name)
        .ok_or_else(|| err(format!("no function named `{name}`")))?;
    if !prog.fns[id].params.is_empty() {
        return Err(err(format!(
            "`{name}` takes arguments; `tez run` needs a zero-argument function"
        )));
    }
    Ok(id)
}

/// Runs every `test` item.
#[must_use]
pub fn run_tests(prog: &Program, opts: &Options) -> Vec<TestResult> {
    prog.tests
        .iter()
        .map(|t| {
            let mut it = Interp::new(prog, opts.clone());
            let mut fr = vec![Value::Unit; t.nlocals as usize];
            let outcome = it
                .eval_public(&t.body, &mut fr)
                .map(|v| matches!(v, Value::Bool(true)));
            TestResult {
                name: t.name.clone(),
                outcome,
            }
        })
        .collect()
}
