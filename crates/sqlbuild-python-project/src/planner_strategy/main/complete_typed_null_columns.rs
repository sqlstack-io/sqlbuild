//! Fixture SQL with its untyped null columns cast to the expected types.

use crate::planner_strategy::_helpers::fixtures::fixture_sql;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Fixture SQL with its untyped null columns cast to the expected types.
#[inline]
pub fn complete_typed_null_columns(
    sql: &str,
    inference: &Bound<'_, PyAny>,
    expected_types: &[(String, String)],
    adapter: &Bound<'_, PyAny>,
) -> PyResult<String> {
    fixture_sql::complete_typed_null_columns(sql, inference, expected_types, adapter)
}
