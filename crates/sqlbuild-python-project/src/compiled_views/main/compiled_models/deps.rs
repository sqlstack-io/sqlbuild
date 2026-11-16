//! The model's dependency `CompiledObjectKey` tuple.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's dependency `CompiledObjectKey` tuple.
#[inline]
pub fn deps<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    model_reads::deps(model)
}
