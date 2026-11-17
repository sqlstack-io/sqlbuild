//! Write `text` to stderr and flush it.

use pyo3::prelude::{PyResult, Python};
use sqlbuild_python_base::native_host::main::write_console::write_console;

/// Write `text` to stderr and flush it.
pub fn write_stderr(py: Python<'_>, text: &str) -> PyResult<()> {
    write_console(py, "stderr", text)
}
