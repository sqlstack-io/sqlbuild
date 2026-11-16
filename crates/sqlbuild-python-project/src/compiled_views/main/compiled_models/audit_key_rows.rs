//! Each audit spec's `(definition name, column argument when it is text)`.

use crate::compiled_views::_helpers::compiled_models::relation_keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Each audit spec's `(definition name, column argument when it is text)`.
#[inline]
pub fn audit_key_rows(audits: &Bound<'_, PyAny>) -> PyResult<Vec<(String, Option<String>)>> {
    relation_keys::audit_key_rows(audits)
}
