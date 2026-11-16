//! Write lasting text, clearing and then redrawing any live transient line.

use crate::build_progress::_helpers::progress;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Write lasting text, clearing and then redrawing any live transient line.
#[inline]
pub fn write_persistent(py: Python<'_>, stream: &Bound<'_, PyAny>, text: &str) -> PyResult<()> {
    progress::write_persistent(py, stream, text)
}
