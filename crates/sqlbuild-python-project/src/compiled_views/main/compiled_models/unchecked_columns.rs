//! Output columns whose types semantic recovery forgot, in any order.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Output columns whose types semantic recovery forgot, in any order.
#[inline]
pub fn unchecked_columns(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    model_reads::unchecked_columns(model)
}
