//! The facts one refactoring plan of `requests` reads.

use crate::refactor::_helpers::refactor_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_refactor::refactoring::models::RefactorFacts;
use sqlbuild_refactor::refactoring::models::RefactorRequest;

/// The facts one refactoring plan of `requests` reads.
#[inline]
pub fn refactor_facts(
    project: &Bound<'_, PyAny>,
    requests: &[RefactorRequest],
) -> PyResult<RefactorFacts> {
    refactor_facts::refactor_facts(project, requests)
}
