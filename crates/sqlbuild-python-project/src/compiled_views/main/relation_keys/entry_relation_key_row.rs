//! The key row of a seed, source or hand-built model from its config values and schema entry.

use crate::compiled_views::_helpers::relation_keys;
use crate::compiled_views::models::RelationKeyRow;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// The key row of a seed, source or hand-built model from its config values and schema entry.
#[inline]
pub fn entry_relation_key_row(
    kind: &str,
    name: String,
    values: &Bound<'_, PyDict>,
    entry: &Bound<'_, PyAny>,
) -> PyResult<RelationKeyRow> {
    relation_keys::entry_relation_key_row(kind, name, values, entry)
}
