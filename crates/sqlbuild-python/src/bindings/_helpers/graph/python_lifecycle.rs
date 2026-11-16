//! Lifecycle regions of a run's selected Python nodes, classified natively.

use std::collections::{BTreeSet, HashMap};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyFrozenSet};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::python_plan::main::run_lifecycle::run_lifecycle;
use sqlbuild_analysis::python_plan::models::{LifecycleNode, RunLifecycle};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const EXTERNAL_CONNECTION_MODE: &str = "external";

/// `build_python_sql_run_lifecycle(*, selection, python_graph)`: a `PythonSqlRunLifecyclePlan`.
#[pyfunction]
#[pyo3(signature = (*, selection, python_graph))]
pub(crate) fn build_python_sql_run_lifecycle<'py>(
    selection: &Bound<'py, PyAny>,
    python_graph: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = selection.py();
    let selected: BTreeSet<String> = native_field(selection, "python_node_names")?
        .try_iter()?
        .map(|name| name?.extract())
        .collect::<PyResult<_>>()?;
    let mut nodes: HashMap<String, LifecycleNode> = HashMap::new();
    for item in native_field(python_graph, "nodes_by_name")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, node): (String, Bound<'py, PyAny>) = item?.extract()?;
        nodes.insert(name, lifecycle_node(&node)?);
    }
    let mut upstream: HashMap<String, Vec<String>> = HashMap::new();
    for item in native_field(python_graph, "upstream_deps")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, deps): (String, Bound<'py, PyAny>) = item?.extract()?;
        let deps: Vec<String> = deps
            .try_iter()?
            .map(|dep| dep?.extract())
            .collect::<PyResult<_>>()?;
        upstream.insert(name, deps);
    }
    let lifecycle: RunLifecycle = py.detach(|| run_lifecycle(&selected, &nodes, &upstream));
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
    new_record(
        py,
        RecordId::PythonSqlRunLifecyclePlan,
        (),
        Some(&arguments),
    )
}

fn lifecycle_node(node: &Bound<'_, PyAny>) -> PyResult<LifecycleNode> {
    let loader: Bound<'_, PyAny> = native_field(node, "loader")?;
    let external_loader: bool = !loader.is_none()
        && native_field(&loader, "connection_mode")?.extract::<String>()?
            == EXTERNAL_CONNECTION_MODE;
    Ok(LifecycleNode {
        kind: native_field(node, "kind")?.extract()?,
        external_loader,
    })
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_python_sql_run_lifecycle, module)?)
}
