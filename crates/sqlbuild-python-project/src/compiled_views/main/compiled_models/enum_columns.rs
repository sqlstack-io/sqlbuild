//! The model's enum columns by name.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's enum columns by name.
#[inline]
pub fn enum_columns<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    model_reads::enum_columns(model)
}
