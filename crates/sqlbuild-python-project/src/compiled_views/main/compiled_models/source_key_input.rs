//! What `relation_keys` reads for a source: its native view itself, or its `source_entry`.

use crate::compiled_views::_helpers::compiled_models::relation_keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// What `relation_keys` reads for a source: its native view itself, or its `source_entry`.
#[inline]
pub fn source_key_input<'py>(source: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    relation_keys::source_key_input(source)
}
