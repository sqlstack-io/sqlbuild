//! The attached-audit gate edges and hook-read edges of one project, in Python's order.

use crate::references::_helpers::explicit_references::gate_row;
use crate::references::_helpers::explicit_references::hook_row;
use crate::references::_helpers::reference_facts::{ProjectFacts, project_facts};
use crate::references::models::GateRow;
use crate::references::models::HookRow;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::explicit_references::main::execution_gate_edges::execution_gate_edges;
use sqlbuild_analysis::explicit_references::models::{ExecutionGateEdges, ExecutionGateRequest};

/// The attached-audit gate edges and hook-read edges of one project, in Python's order.
pub fn native_execution_gate_edges(
    project: &Bound<'_, PyAny>,
) -> PyResult<(Vec<GateRow>, Vec<HookRow>)> {
    let facts: ProjectFacts = project_facts(project, false)?;
    let edges: ExecutionGateEdges = execution_gate_edges(&ExecutionGateRequest {
        resources: facts.resources,
        models: facts.models,
        audits: facts.audits,
        hooks: facts.hooks,
    });
    Ok((
        edges.audit_gates.into_iter().map(gate_row).collect(),
        edges.hook_reads.into_iter().map(hook_row).collect(),
    ))
}
