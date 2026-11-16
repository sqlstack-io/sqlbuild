//! Model change.

use crate::planner_changes::_helpers::detect;
use crate::planner_changes::models::ModelPass;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Model change.
#[inline]
pub fn model_change<'py>(
    pass: &ModelPass<'py>,
    model: &Bound<'py, PyAny>,
    (expected_version_hash, expected_metadata_json): (Option<String>, Option<String>),
) -> PyResult<Bound<'py, PyAny>> {
    detect::model_change(pass, model, (expected_version_hash, expected_metadata_json))
}
