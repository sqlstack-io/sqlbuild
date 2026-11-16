//! Declarations.

use crate::compiled_project::_helpers::project;
use crate::compiled_project::models::DeclarationPy;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::compiled_project::models::DeclarationFact;

/// Declarations.
#[inline]
pub fn declarations(rows: Vec<DeclarationPy<'_>>) -> PyResult<Vec<DeclarationFact>> {
    project::declarations(rows)
}
