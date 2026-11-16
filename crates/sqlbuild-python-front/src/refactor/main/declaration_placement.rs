//! Where scope placement puts declarations once every `(model, destination)` moves together.

use crate::refactor::_helpers::refactor_facts;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_refactor::refactoring::models::DeclarationPlacement;
use std::collections::BTreeSet;

/// Where scope placement puts declarations once every `(model, destination)` moves together.
#[inline]
pub fn declaration_placement(
    project: &Bound<'_, PyAny>,
    moves: &[(String, String)],
    vacated: &BTreeSet<String>,
) -> PyResult<DeclarationPlacement> {
    refactor_facts::declaration_placement(project, moves, vacated)
}
