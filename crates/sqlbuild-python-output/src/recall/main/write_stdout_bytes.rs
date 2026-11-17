//! Write `data` unchanged to stdout's binary buffer after flushing pending text.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBytes;

/// Write `data` unchanged to stdout's binary buffer after flushing pending text.
pub fn write_stdout_bytes(py: Python<'_>, data: &[u8]) -> PyResult<()> {
    let stdout: Bound<'_, PyAny> = py.import("sys")?.getattr("stdout")?;
    stdout.call_method0("flush")?;
    let target: Bound<'_, PyAny> = stdout.getattr("buffer").unwrap_or(stdout);
    target.call_method1("write", (PyBytes::new(py, data),))?;
    target.call_method0("flush")?;
    Ok(())
}
