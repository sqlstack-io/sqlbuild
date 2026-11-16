//! Each reference of `model_input` as `(kind, name, package, call argument count)`.

use crate::compiled_views::_helpers::model_inputs::model_input_view;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_project::models::ReferenceRow;

/// Each reference of `model_input` as `(kind, name, package, call argument count)`.
#[inline]
pub fn input_reference_rows(model_input: &Bound<'_, PyAny>) -> PyResult<Vec<ReferenceRow>> {
    model_input_view::input_reference_rows(model_input)
}
