//! Lifecycle regions of a run's selected Python nodes, classified natively.

use std::collections::{BTreeSet, HashMap};

use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::python_plan::main::run_lifecycle::run_lifecycle;
use sqlbuild_analysis::python_plan::models::{LifecycleNode, RunLifecycle};

/// `build_python_sql_run_lifecycle(*, selection, python_graph)`: a `PythonSqlRunLifecyclePlan`.
#[pyfunction]
#[pyo3(signature = (*, selection, python_graph))]
pub(crate) fn build_python_sql_run_lifecycle<'py>(
    selection: &Bound<'py, PyAny>,
    python_graph: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = selection.py();
    let lifecycle: RunLifecycle = native_run_lifecycle(selection, python_graph)?;
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item(
        "ingress_python_node_names",
        PyFrozenSet::new(py, lifecycle.ingress_names)?,
    )?;
    arguments.set_item(
        "ingress_loader_names",
        PyFrozenSet::new(py, lifecycle.ingress_loader_names)?,
    )?;
    arguments.set_item(
        "read_side_python_node_names",
        PyFrozenSet::new(py, lifecycle.read_side_names)?,
    )?;
    arguments.set_item(
        "round_trip_loader_names",
        PyFrozenSet::new(
            py,
            lifecycle
                .round_trip_loaders
                .into_iter()
                .map(|loader| loader.name),
        )?,
    )?;
    new_record(
        py,
        RecordId::PythonSqlRunLifecyclePlan,
        (),
        Some(&arguments),
    )
}

/// The lifecycle regions of a `PythonSqlRunSelection` over a `PythonNodeGraph`.
pub(crate) fn native_run_lifecycle(
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

fn lifecycle_node(node: &Bound<'_, PyAny>) -> PyResult<LifecycleNode> {
    let mut sql_deps: Vec<ObjectKey> = Vec::new();
    for reference in native_field(node, "sql_deps")?.try_iter()? {
        let reference: Bound<'_, PyAny> = reference?;
        sql_deps.push((
            native_field(&reference, "kind")?.extract()?,
            native_field(&reference, "name")?.extract()?,
        ));
    }
    Ok(LifecycleNode {
        kind: native_field(node, "kind")?.extract()?,
        sql_deps,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_python_sql_run_lifecycle, module)?)
}
