//! Injected-edge descriptions remain native for cycle and buildability diagnostics.

use crate::planner_identity::main::execution_origins_with::execution_origins_with;
use crate::references::main::native_execution_gate_edges::native_execution_gate_edges;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Injected-edge descriptions remain native for cycle and buildability diagnostics.
pub fn execution_origins(
    project: &Bound<'_, PyAny>,
) -> PyResult<Vec<((ObjectKey, ObjectKey), String)>> {
    execution_origins_with(project, &native_execution_gate_edges(project)?)
}
