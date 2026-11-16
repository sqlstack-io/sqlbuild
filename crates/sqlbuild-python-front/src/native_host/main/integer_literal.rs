//! Integer literal.

use crate::native_host::_helpers::consumption_state::state_literal;
use pyo3::prelude::{PyResult, Python};
use pyo3::types::PyInt;
use sqlbuild_state::consumption::constants::INTEGER_STATE_VALUE_TYPE;

/// Integer literal.
pub fn integer_literal(py: Python<'_>, value: Option<i64>) -> PyResult<String> {
    state_literal(
        py,
        value.map(|value| PyInt::new(py, value).into_any()),
        INTEGER_STATE_VALUE_TYPE,
    )
}
