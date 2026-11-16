//! Python's `_escaped`: undecodable name bytes as `\xNN`, other lone surrogates as `\uXXXX`.

use crate::discovery::_helpers::snapshots::snapshot;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyString;

/// Python's `_escaped`: undecodable name bytes as `\xNN`, other lone surrogates as `\uXXXX`.
#[inline]
pub fn escaped(text: &Bound<'_, PyString>) -> PyResult<String> {
    snapshot::escaped(text)
}
