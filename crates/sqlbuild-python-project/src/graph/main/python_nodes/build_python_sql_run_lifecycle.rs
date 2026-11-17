//! `build_python_sql_run_lifecycle(*, selection, python_graph)`: a `PythonSqlRunLifecyclePlan`.

use crate::graph::_helpers::python_nodes::python_lifecycle::lifecycle_node;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyFrozenSet};
use sqlbuild_analysis::python_plan::main::run_lifecycle::run_lifecycle;
use sqlbuild_analysis::python_plan::models::{LifecycleNode, RunLifecycle};
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use std::collections::{BTreeSet, HashMap};
use crate::graph::main::python_nodes::native_run_lifecycle::native_run_lifecycle;
use sqlbuild_analysis::python_plan::models::RunLifecycle;

/// `build_python_sql_run_lifecycle(*, selection, python_graph)`: a `PythonSqlRunLifecyclePlan`.
#[pyfunction]
#[pyo3(signature = (*, selection, python_graph))]
pub fn build_python_sql_run_lifecycle<'py>(
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
