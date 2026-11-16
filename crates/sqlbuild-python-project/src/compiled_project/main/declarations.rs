//! Declarations.

use crate::compiled_project::_helpers::project::declaration;
use crate::compiled_project::models::DeclarationPy;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::compiled_project::models::DeclarationFact;

/// Declarations.
pub fn declarations(rows: Vec<DeclarationPy<'_>>) -> PyResult<Vec<DeclarationFact>> {
    rows.into_iter().map(declaration).collect()
}
