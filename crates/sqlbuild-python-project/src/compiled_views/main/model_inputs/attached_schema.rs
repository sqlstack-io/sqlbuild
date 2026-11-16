//! The attached schema of `model_input`; a native input answers without building its entry.

use crate::compiled_views::_helpers::model_inputs::model_input_view;
use crate::compiled_views::models::AttachedSchema;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The attached schema of `model_input`; a native input answers without building its entry.
#[inline]
pub fn attached_schema<'py>(model_input: &Bound<'py, PyAny>) -> PyResult<Option<AttachedSchema>> {
    model_input_view::attached_schema(model_input)
}
