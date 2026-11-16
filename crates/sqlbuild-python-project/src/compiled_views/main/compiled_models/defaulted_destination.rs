//! The destination with adapter defaults filled in; `render` renders its qualified name.

use crate::compiled_views::_helpers::compiled_models::compiled_model_view;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_project::models::DestinationFact;

/// The destination with adapter defaults filled in; `render` renders its qualified name.
#[inline]
pub fn defaulted_destination(
    current: &DestinationFact,
    defaults: (Option<String>, Option<String>),
    render: &Bound<'_, PyAny>,
) -> PyResult<DestinationFact> {
    compiled_model_view::defaulted_destination(current, defaults, render)
}
