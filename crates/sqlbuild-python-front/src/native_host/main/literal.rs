//! Literal.

use crate::native_host::_helpers::consumption_state::state_literal;
use crate::native_host::_helpers::consumption_state::text;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_state::freshness::constants::STRING_STATE_VALUE_TYPE;

/// Literal.
pub fn literal(py: Python<'_>, value: Option<&str>) -> PyResult<String> {
    state_literal(
        py,
        value.map(|value| text(py, value)),
        STRING_STATE_VALUE_TYPE,
    )
}
