//! Digest the whole context a macro may read, or None when a value has no stable text.

use crate::macro_bridge::_helpers::store_keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Digest the whole context a macro may read, or None when a value has no stable text.
#[inline]
pub fn context_store_token<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<String>> {
    store_keys::context_store_token(py, options)
}
