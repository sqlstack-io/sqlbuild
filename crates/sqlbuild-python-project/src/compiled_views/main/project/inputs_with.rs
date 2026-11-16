//! The compile inputs with `changes`; a native view shares its owner's model inputs.

use crate::compiled_views::_helpers::project::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// The compile inputs with `changes`; a native view shares its owner's model inputs.
#[inline]
pub fn inputs_with<'py>(
    inputs: &Bound<'py, PyAny>,
    changes: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    records::inputs_with(inputs, changes)
}
