//! Model facts.

use crate::compiled_project::_helpers::project;
use crate::compiled_project::_helpers::project::ModelInputPy;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;

/// Model facts.
#[inline]
pub fn model_facts(model: ModelInputPy<'_>) -> PyResult<CompiledModelFacts> {
    project::model_facts(model)
}
