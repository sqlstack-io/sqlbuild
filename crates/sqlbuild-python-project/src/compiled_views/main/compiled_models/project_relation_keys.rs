//! Declared keys of a compiled project's models, seeds and sources.

use crate::compiled_views::_helpers::compiled_models::relation_keys;
use crate::compiled_views::types::RelationKeys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Declared keys of a compiled project's models, seeds and sources.
#[inline]
pub fn project_relation_keys(project: &Bound<'_, PyAny>) -> PyResult<RelationKeys> {
    relation_keys::project_relation_keys(project)
}
