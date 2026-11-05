//! Recall's writes to the process streams, through Python so captures and tees see them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBytes;

use crate::bindings::_helpers::native_host::host::write_console;

/// Write `data` unchanged to stdout's binary buffer after flushing pending text.
pub(crate) fn write_stdout_bytes(py: Python<'_>, data: &[u8]) -> PyResult<()> {
    let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
    stdout.call_method0("flush")?;
    let target: Bound<'_, PyAny> = stdout.getattr("buffer").unwrap_or(stdout);
    target.call_method1("write", (PyBytes::new(py, data),))?;
    target.call_method0("flush")?;
    Ok(())
}

/// Write `text` to stderr and flush it.
pub(crate) fn write_stderr(py: Python<'_>, text: &str) -> PyResult<()> {
    write_console(py, "stderr", text)
}
