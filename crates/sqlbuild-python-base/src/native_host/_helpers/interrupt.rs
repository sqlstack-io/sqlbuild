//! Python's default compilation Ctrl+C handler, retained across unraisable callbacks.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::{Py, PyAny, PyAnyMethods, PyModule, PyResult, Python};

thread_local! {
    pub(crate) static INTERRUPTED: Cell<bool> = const { Cell::new(false) };
}
pub(crate) const SIGINT: i64 = 2;
/// Whether Ctrl+C reached the `sqb` process's watch, which sees it before any handler runs.
pub(crate) static WATCHED_INTERRUPT: AtomicBool = AtomicBool::new(false);

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

/// Put `signal`'s previous Python handler back, unless something replaced `installed` meanwhile. Fails when a pending handler raised first, as `signal.signal` runs pending handlers before it changes one.
pub(crate) fn restore_signal_handler(
    py: Python<'_>,
    signal: i64,
    (installed, previous): (&Py<PyAny>, &Py<PyAny>),
) -> PyResult<()> {
    let module: pyo3::Bound<'_, PyModule> = py.import("signal")?;
    if module
        .call_method1("getsignal", (signal,))?
        .is(installed.bind(py))
    {
        module.call_method1("signal", (signal, previous.bind(py)))?;
    }
    Ok(())
}

/// Whether the default handler or the process's watch observed Ctrl+C during the current command.
pub(crate) fn interrupt_observed() -> bool {
    interrupted() || WATCHED_INTERRUPT.load(Ordering::SeqCst)
}
