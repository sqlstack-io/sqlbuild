//! Python's default compilation Ctrl+C handler, retained across unraisable callbacks.

use std::cell::Cell;

use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyCFunction, PyDict, PyTuple};

thread_local! {
    static INTERRUPTED: Cell<bool> = const { Cell::new(false) };
}
const SIGINT: i64 = 2;

/// The previous Python handler and the command's replacement, restored on every exit path.
pub(crate) struct InterruptHandler {
    previous: Py<PyAny>,
    installed: Py<PyAny>,
}

impl InterruptHandler {
    /// Capture only Python's default main-thread handler; leave embedding applications' handlers alone.
    pub(crate) fn install(py: Python<'_>) -> PyResult<Option<Self>> {
        let threading = py.import("threading")?;
        if !threading
            .call_method0("current_thread")?
            .is(threading.call_method0("main_thread")?)
        {
            return Ok(None);
        }
        let signal = py.import("signal")?;
        let previous = signal.call_method1("getsignal", (SIGINT,))?;
        if !previous.is(signal.getattr("default_int_handler")?) {
            return Ok(None);
        }
        let installed = PyCFunction::new_closure(
            py,
            None,
            None,
            |_: &Bound<'_, PyTuple>, _: Option<&Bound<'_, PyDict>>| -> PyResult<()> {
                INTERRUPTED.set(true);
                Err(PyKeyboardInterrupt::new_err(()))
            },
        )?;
        INTERRUPTED.set(false);
        signal.call_method1("signal", (SIGINT, &installed))?;
        Ok(Some(Self {
            previous: previous.unbind(),
            installed: installed.into_any().unbind(),
        }))
    }
}

impl Drop for InterruptHandler {
    fn drop(&mut self) {
        Python::attach(|py| {
            let restored: PyResult<()> = (|| {
                let signal = py.import("signal")?;
                if signal
                    .call_method1("getsignal", (SIGINT,))?
                    .is(self.installed.bind(py))
                {
                    signal.call_method1("signal", (SIGINT, self.previous.bind(py)))?;
                }
                Ok(())
            })();
            drop(restored);
        });
        INTERRUPTED.set(false);
    }
}

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
