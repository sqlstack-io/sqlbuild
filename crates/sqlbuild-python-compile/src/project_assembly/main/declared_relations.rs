//! Declared column types, declared nullability and the authoritative binding schemas.

use crate::project_assembly::_helpers::declared_relations;
use crate::project_assembly::models::DeclaredRelations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Declared column types, declared nullability and the authoritative binding schemas.
#[inline]
pub fn declared_relations<'py>(inputs: Bound<'py, PyAny>) -> PyResult<DeclaredRelations<'py>> {
    declared_relations::declared_relations(inputs)
}
