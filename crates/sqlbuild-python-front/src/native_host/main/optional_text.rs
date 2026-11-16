//! Optional text.

use crate::native_host::_helpers::consumption_state;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Optional text.
#[inline]
pub fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    consumption_state::optional_text(value)
}
