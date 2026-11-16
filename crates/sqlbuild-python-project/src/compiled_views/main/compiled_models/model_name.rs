//! The model's name.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's name.
#[inline]
pub fn model_name(model: &Bound<'_, PyAny>) -> PyResult<String> {
    model_reads::model_name(model)
}
