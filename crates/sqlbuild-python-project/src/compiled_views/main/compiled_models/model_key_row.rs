//! The declared key facts of a model input or compiled model.

use crate::compiled_views::_helpers::compiled_models::relation_keys;
use crate::compiled_views::models::RelationKeyRow;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The declared key facts of a model input or compiled model.
#[inline]
pub fn model_key_row(model: &Bound<'_, PyAny>) -> PyResult<RelationKeyRow> {
    relation_keys::model_key_row(model)
}
