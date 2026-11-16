//! The compile inputs with `changes`; a native view shares its owner's model inputs.

use crate::compiled_views::_helpers::project::records::with_changes;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_python_base::records::types::RecordId;

/// The compile inputs with `changes`; a native view shares its owner's model inputs.
#[pyfunction]
#[pyo3(signature = (*, inputs, **changes))]
pub fn inputs_with<'py>(
    inputs: &Bound<'py, PyAny>,
    changes: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    with_changes(inputs, RecordId::CompileProjectInputs, changes)
}
