//! Whether the model's dynamic pivot proof proved its output columns.

use crate::compiled_views::_helpers::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Whether the model's dynamic pivot proof proved its output columns.
#[inline]
pub fn output_proven(model: &Bound<'_, PyAny>) -> PyResult<bool> {
    model_reads::output_proven(model)
}
