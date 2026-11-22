//! Write a line to Python's stderr for other domains, as the plan's own progress is written.

use pyo3::prelude::{PyResult, Python};

use crate::native_host::_helpers::synced_catalog::write_stderr;

/// `text` ends in its own newline.
pub fn write_stderr_line(py: Python<'_>, text: &str) -> PyResult<()> {
    write_stderr(py, text)
}
