//! A snapshot model's validity column names, which the snapshot adds to its query's columns.

use crate::semantic_shapes::_helpers::shapes;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A snapshot model's validity column names, which the snapshot adds to its query's columns.
#[inline]
pub fn snapshot_validity_columns(model: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    shapes::snapshot_validity_columns(model)
}
