//! Relocated records after every move in `moves`, and per record whether it lacks a needed group.

use crate::scopes::_helpers::checks::scope_validation;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyTuple;
use sqlbuild_scopes::scope_index::models::ResourceIdentity;

/// Relocated records after every move in `moves`, and per record whether it lacks a needed group.
#[inline]
pub fn relocated_declarations<'py>(
    index: &Bound<'py, PyAny>,
    moves: &[(ResourceIdentity, String)],
) -> PyResult<Option<(Bound<'py, PyTuple>, Vec<bool>)>> {
    scope_validation::relocated_declarations(index, moves)
}
