//! The declared column facts of `model_input`'s attached schema, empty without one.

use crate::compiled_views::_helpers::model_inputs::model_input_view;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;

/// The declared column facts of `model_input`'s attached schema, empty without one.
#[inline]
pub fn attached_columns(model_input: &Bound<'_, PyAny>) -> PyResult<Vec<SchemaColumnFact>> {
    model_input_view::attached_columns(model_input)
}
