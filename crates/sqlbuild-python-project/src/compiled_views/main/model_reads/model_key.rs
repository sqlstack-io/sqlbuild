//! The model's `CompiledObjectKey`.

use crate::compiled_views::_helpers::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's `CompiledObjectKey`.
#[inline]
pub fn model_key<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    model_reads::model_key(model)
}
