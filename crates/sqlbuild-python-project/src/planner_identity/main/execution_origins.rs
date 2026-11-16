//! Injected-edge descriptions remain native for cycle and buildability diagnostics.

use crate::planner_identity::_helpers::graph;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Injected-edge descriptions remain native for cycle and buildability diagnostics.
#[inline]
pub fn execution_origins(
    project: &Bound<'_, PyAny>,
) -> PyResult<Vec<((ObjectKey, ObjectKey), String)>> {
    graph::execution_origins(project)
}
