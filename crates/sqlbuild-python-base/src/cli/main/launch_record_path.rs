//! The offered launch record path, unless an input outside its key was read.

use crate::cli::_helpers::launch_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The offered launch record path, unless an input outside its key was read.
#[inline]
pub fn launch_record_path(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    launch_state::launch_record_path(py)
}
