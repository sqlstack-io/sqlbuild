//! `value` as its state column's literal, timestamps cast to `timestamp_type` when given.

use crate::contracts::_helpers::state_literals;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `value` as its state column's literal, timestamps cast to `timestamp_type` when given.
#[inline]
pub fn render_state_sql_literal(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
    declared_type: &str,
    timestamp_type: Option<&str>,
) -> PyResult<String> {
    state_literals::render_state_sql_literal(py, value, declared_type, timestamp_type)
}
