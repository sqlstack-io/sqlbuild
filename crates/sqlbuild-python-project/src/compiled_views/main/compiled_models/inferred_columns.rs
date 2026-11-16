//! The model's `InferredColumn` tuple, or `None`.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's `InferredColumn` tuple, or `None`.
#[inline]
pub fn inferred_columns<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    model_reads::inferred_columns(model)
}
