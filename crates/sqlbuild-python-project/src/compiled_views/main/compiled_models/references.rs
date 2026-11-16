//! The model's `CompileSqlReference` tuple.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's `CompileSqlReference` tuple.
#[inline]
pub fn references<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    model_reads::references(model)
}
