//! Name a macro by its name, file, identity and exact source, or None when it has no key.

use crate::macro_bridge::_helpers::store_keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Name a macro by its name, file, identity and exact source, or None when it has no key.
#[inline]
pub fn macro_store_token<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Option<String>> {
    store_keys::macro_store_token(py, options)
}
