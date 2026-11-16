//! The relation a model or seed destination names; a destination without a schema has an empty one.

use crate::consumption::_helpers::relation_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_state::consumption::models::RelationKey;

/// The relation a model or seed destination names; a destination without a schema has an empty one.
#[inline]
pub fn relation(destination: &Bound<'_, PyAny>) -> PyResult<RelationKey> {
    relation_reads::relation(destination)
}
