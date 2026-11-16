//! Whether the native session validated the model's bindings.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether the native session validated the model's bindings.
#[inline]
pub fn binding_validated(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    model_reads::binding_validated(model)
}
