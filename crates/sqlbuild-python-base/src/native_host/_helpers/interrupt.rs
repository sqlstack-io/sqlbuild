//! Python's default compilation Ctrl+C handler, retained across unraisable callbacks.

use std::cell::Cell;

use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::{PyResult, Python};

thread_local! {
    pub(crate) static INTERRUPTED: Cell<bool> = const { Cell::new(false) };
}
pub(crate) const SIGINT: i64 = 2;

/// Whether the default handler observed Ctrl+C during the current command.
pub(crate) fn interrupted() -> bool {
    INTERRUPTED.get()
}

/// Check both pending Python signals and a Ctrl+C whose exception a Python callback swallowed.
pub(crate) fn check_signals(py: Python<'_>) -> PyResult<()> {
    py.check_signals()?;
    if interrupted() {
        return Err(PyKeyboardInterrupt::new_err(()));
    }
    Ok(())
}
