//! `native_run_lifecycle(selection, python_graph)`: the lifecycle regions of a run's Python nodes.

use crate::graph::_helpers::python_nodes::python_lifecycle::lifecycle_node;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_analysis::python_plan::main::run_lifecycle::run_lifecycle;
use sqlbuild_analysis::python_plan::models::{LifecycleNode, RunLifecycle};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::{BTreeSet, HashMap};

/// The lifecycle regions of a `PythonSqlRunSelection` over a `PythonNodeGraph`.
pub fn native_run_lifecycle(
    selection: &Bound<'_, PyAny>,
    python_graph: &Bound<'_, PyAny>,
) -> PyResult<RunLifecycle> {
    let py: Python<'_> = selection.py();
    let selected: BTreeSet<String> = native_field(selection, "python_node_names")?
        .try_iter()?
        .map(|name| name?.extract())
        .collect::<PyResult<_>>()?;
    let mut nodes: HashMap<String, LifecycleNode> = HashMap::new();
    for item in native_field(python_graph, "nodes_by_name")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, node): (String, Bound<'_, PyAny>) = item?.extract()?;
        nodes.insert(name, lifecycle_node(&node)?);
    }
    let mut upstream: HashMap<String, Vec<String>> = HashMap::new();
    for item in native_field(python_graph, "upstream_deps")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, deps): (String, Bound<'_, PyAny>) = item?.extract()?;
        let deps: Vec<String> = deps
            .try_iter()?
            .map(|dep| dep?.extract())
            .collect::<PyResult<_>>()?;
        upstream.insert(name, deps);
    }
    Ok(py.detach(|| run_lifecycle(&selected, &nodes, &upstream)))
}
