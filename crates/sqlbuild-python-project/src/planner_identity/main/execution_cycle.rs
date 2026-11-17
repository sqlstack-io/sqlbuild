//! `execution_cycle(project)`: the execution-order cycle every plan refuses with.

use crate::planner_identity::_helpers::execution::ordered_keys;
use crate::planner_identity::main::execution_deps_with::execution_deps_with;
use crate::planner_identity::main::execution_origins_with::execution_origins_with;
use crate::references::main::native_execution_gate_edges::native_execution_gate_edges;
use crate::references::types::ExecutionGates;
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
