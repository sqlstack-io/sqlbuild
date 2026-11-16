//! Keys proving one row per key by `kind:name`; seeds in `one_row_seeds` also get the empty key.

use crate::compiled_views::_helpers::compiled_models::relation_keys;
use crate::compiled_views::models::RelationKeyRow;
use crate::compiled_views::types::RelationKeys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Keys proving one row per key by `kind:name`; seeds in `one_row_seeds` also get the empty key.
#[inline]
pub fn relation_keys<'py>(
    py: Python<'py>,
    rows: Vec<RelationKeyRow>,
    (seeds, sources): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    one_row_seeds: &[String],
) -> PyResult<RelationKeys> {
    relation_keys::relation_keys(py, rows, (seeds, sources), one_row_seeds)
}
