//! The name of `model_input`, its file stem; a native input answers from its record.

use crate::compiled_views::_helpers::model_input_view;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The name of `model_input`, its file stem; a native input answers from its record.
#[inline]
pub fn input_model_name(model_input: &Bound<'_, PyAny>) -> PyResult<String> {
    model_input_view::input_model_name(model_input)
}
