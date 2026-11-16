//! Remember where `sqb` looks for this invocation's launch record, or that it offered none.

use crate::cli::_helpers::launch_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Remember where `sqb` looks for this invocation's launch record, or that it offered none.
#[inline]
pub fn offer_launch_record(py: Python<'_>, path: Option<Bound<'_, PyAny>>) -> PyResult<()> {
    launch_state::offer_launch_record(py, path)
}
