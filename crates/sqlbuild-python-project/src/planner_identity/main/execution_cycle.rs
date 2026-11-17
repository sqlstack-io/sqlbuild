//! The cycle every plan refuses with, since plans order the whole execution graph before selecting.

use crate::planner_identity::_helpers::graph::ExecutionGates;
use crate::planner_identity::_helpers::graph::execution_deps_with;
use crate::planner_identity::_helpers::graph::execution_origins_with;
use crate::planner_identity::_helpers::graph::ordered_keys;
use crate::references::main::native_execution_gate_edges::native_execution_gate_edges;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// The cycle every plan refuses with, since plans order the whole execution graph before selecting.
pub fn execution_cycle(project: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let gates: ExecutionGates = native_execution_gate_edges(project)?;
    Ok(ordered_keys(
        &execution_deps_with(project, &gates)?,
        &execution_origins_with(project, &gates)?,
    )
    .err())
}
