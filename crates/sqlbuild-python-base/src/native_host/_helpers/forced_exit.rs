//! Ending an interrupted process without waiting for what is left in it.

use std::io::{IsTerminal, Write};
use std::sync::atomic::Ordering;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::boundary::main::imported::imported;
use crate::native_host::_helpers::terminal_settings::restore_terminal;
use crate::native_host::constants::{FORCED_EXIT_WAIT, INTERRUPTED_CODE, INTERRUPTED_LINE};
use crate::native_host::main::host_entry::host_entry;
use crate::native_host::models::InterruptWatch;

/// Start `target` on a Python daemon thread, which interpreter exit doesn't wait for.
fn background<'py>(target: Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target.py();
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("target", target)?;
    options.set_item("daemon", true)?;
    let thread: Bound<'py, PyAny> =
        imported(py, "threading", "Thread")?.call((), Some(&options))?;
    thread.call_method0("start")?;
    Ok(thread)
}

/// Cancel every running statement without waiting for the warehouse's answer.
pub(crate) fn cancel_statements(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    background(host_entry(py, ExecutorHost::CancelRunningStatements)?)
}

/// The one line an interrupted command ends with.
pub(crate) fn announce(watch: &InterruptWatch) {
    if watch.announced.swap(true, Ordering::SeqCst) {
        return;
    }
    let mut stderr: std::io::Stderr = std::io::stderr();
    let lead: &str = if stderr.is_terminal() { "\n" } else { "" };
    drop(writeln!(stderr, "{lead}{INTERRUPTED_LINE}"));
}

/// Cancel statements, flush, restore the terminal, say so and end the process with 130.
pub(crate) fn force_exit(py: Python<'_>, watch: &InterruptWatch) -> PyResult<()> {
    let wait = |thread: PyResult<Bound<'_, PyAny>>| {
        drop(
            thread
                .and_then(|thread| thread.call_method1("join", (FORCED_EXIT_WAIT.as_secs_f64(),))),
        );
    };
    wait(cancel_statements(py));
    wait(
        imported(py, "sys", "stdout")
            .and_then(|stdout| stdout.getattr("flush"))
            .and_then(background),
    );
    if let Some(terminal) = &watch.terminal {
        drop(restore_terminal(py, terminal));
    }
    announce(watch);
    imported(py, "os", "_exit")?.call1((INTERRUPTED_CODE,))?;
    Ok(())
}
