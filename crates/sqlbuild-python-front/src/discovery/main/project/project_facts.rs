//! The facts of `inputs`, with the Python nodes the facade has described.

use crate::discovery::_helpers::project::project_facts;
use crate::discovery::models::NodeRow;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_discovery::project_validation::models::ProjectFacts;

/// The facts of `inputs`, with the Python nodes the facade has described.
#[inline]
pub fn project_facts(
    py: Python<'_>,
    inputs: &Bound<'_, PyAny>,
    nodes: Vec<NodeRow>,
) -> PyResult<ProjectFacts> {
    project_facts::project_facts(py, inputs, nodes)
}
