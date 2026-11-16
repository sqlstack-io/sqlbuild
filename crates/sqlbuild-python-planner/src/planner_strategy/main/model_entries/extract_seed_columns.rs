//! Declared seed columns with types, as `ColumnInfo`.

use crate::planner_strategy::_helpers::model_entries::schema_columns;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyTuple;

/// Declared seed columns with types, as `ColumnInfo`.
pub fn extract_seed_columns<'py>(seed: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    schema_columns(seed, true)
}
