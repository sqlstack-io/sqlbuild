//! `(resource type, name)` of each dependency.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `(resource type, name)` of each dependency.
#[inline]
pub fn dep_rows(model: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    model_reads::dep_rows(model)
}
