//! Every compiled model's refactoring facts, or none without a project.

use crate::refactor::_helpers::refactor_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_refactor::refactoring::models::ModelFacts;

/// Every compiled model's refactoring facts, or none without a project.
#[inline]
pub fn model_facts(refactor_project: &Bound<'_, PyAny>) -> PyResult<Option<Vec<ModelFacts>>> {
    refactor_facts::model_facts(refactor_project)
}
