//! The input relations of a dynamic pivot proof that proved the model's outputs, else `None`.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The input relations of a dynamic pivot proof that proved the model's outputs, else `None`.
#[inline]
pub fn proven_inputs(model: &Bound<'_, PyAny>) -> PyResult<Option<Vec<String>>> {
    model_reads::proven_inputs(model)
}
