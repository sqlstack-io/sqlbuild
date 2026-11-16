//! The hash of a model's own declared columns, column families and contract setting.

use crate::planner_identity::_helpers::metadata;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The hash of a model's own declared columns, column families and contract setting.
#[inline]
pub fn model_declared_columns_hash(model: &Bound<'_, PyAny>) -> PyResult<String> {
    metadata::model_declared_columns_hash(model)
}
