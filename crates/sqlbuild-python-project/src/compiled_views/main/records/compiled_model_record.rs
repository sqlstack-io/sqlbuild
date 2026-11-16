//! The model as a `CompiledModel`, projected from its owner when it is a native view.

use crate::compiled_views::_helpers::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model as a `CompiledModel`, projected from its owner when it is a native view.
#[inline]
pub fn compiled_model_record<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    records::compiled_model_record(model)
}
