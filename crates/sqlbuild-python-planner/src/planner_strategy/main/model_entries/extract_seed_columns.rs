//! Declared seed columns with types, as `ColumnInfo`.

use crate::planner_strategy::_helpers::model_entries;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;

/// Declared seed columns with types, as `ColumnInfo`.
#[inline]
pub fn extract_seed_columns<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    model_entries::extract_seed_columns(seed)
}
