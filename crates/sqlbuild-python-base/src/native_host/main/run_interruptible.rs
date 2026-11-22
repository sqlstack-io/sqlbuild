//! `run_interruptible`: one command in the `sqb` process, which Ctrl+C always stops.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};

use crate::native_host::_helpers::watch_session::{settle, start_watching};

/// Call `run` and return its exit code, or 130 once Ctrl+C stopped it.
pub fn run_interruptible(run: &Bound<'_, PyAny>, terminated: fn() -> bool) -> PyResult<i64> {
    let py: Python<'_> = run.py();
    let Some(session) = start_watching(py, terminated)? else {
        return run.call0()?.extract();
    };
    let ran: PyResult<i64> = run.call0().and_then(|code| code.extract());
    settle(py, session, ran)
}
