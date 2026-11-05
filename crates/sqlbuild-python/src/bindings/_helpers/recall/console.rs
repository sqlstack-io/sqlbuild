//! Recall's writes to the process streams, through Python so captures and tees see them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBytes;

/// Write `data` unchanged to stdout's binary buffer after flushing pending text.
pub(crate) fn write_stdout_bytes(py: Python<'_>, data: &[u8]) -> PyResult<()> {
    let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
    stdout.call_method0("flush")?;
    let target: Bound<'_, PyAny> = stdout.getattr("buffer").unwrap_or(stdout);
    target.call_method1("write", (PyBytes::new(py, data),))?;
    target.call_method0("flush")?;
    Ok(())
}

/// Write `text` to the named `sys` stream and flush it, so redirections and captures see it.
pub(crate) fn write_text(py: Python<'_>, stream: &str, text: &str) -> PyResult<()> {
    let stream: Bound<'_, PyAny> = py.import("sys")?.getattr(stream)?;
    stream.call_method1("write", (text,))?;
    stream.call_method0("flush")?;
    Ok(())
}

/// Write `text` to stderr and flush it.
pub(crate) fn write_stderr(py: Python<'_>, text: &str) -> PyResult<()> {
    write_text(py, "stderr", text)
}
