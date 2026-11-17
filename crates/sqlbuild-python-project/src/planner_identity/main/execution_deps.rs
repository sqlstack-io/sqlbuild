//! Execution dependencies kept native until a public graph or plan asks for its dict view.

use crate::planner_identity::main::execution_deps_with::execution_deps_with;
use crate::references::main::native_execution_gate_edges::native_execution_gate_edges;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Execution dependencies kept native until a public graph or plan asks for its dict view.
pub fn execution_deps(project: &Bound<'_, PyAny>) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    execution_deps_with(project, &native_execution_gate_edges(project)?)
}
