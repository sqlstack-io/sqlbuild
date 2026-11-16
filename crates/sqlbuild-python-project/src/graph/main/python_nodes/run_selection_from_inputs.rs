//! `PythonSqlRunSelection` of run selectors over the project graph and discovered Python nodes.

use crate::graph::_helpers::python_nodes::python_selection::resolve_python_sql_run_selection;
use crate::graph::main::project::compiled_graph_keys::compiled_graph_keys;
use crate::graph::main::python_nodes::python_node_graph::python_node_graph;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyFrozenSet;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// `PythonSqlRunSelection` of run selectors over the project graph and discovered Python nodes.
pub fn run_selection_from_inputs<'py>(
    (select, exclude): (Vec<String>, Vec<String>),
    project_graph: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_graph.py();
    let python_graph: Bound<'py, PyAny> = python_node_graph(discovered_inputs)?;
    let (sql_keys, python_node_names) =
        resolve_python_sql_run_selection(project_graph, &python_graph, select, exclude)?;
    new_record_with(
        py,
        RecordId::PythonSqlRunSelection,
        &[
            ("sql_keys", compiled_graph_keys(py, sql_keys)?.into_any()),
            (
                "python_node_names",
                PyFrozenSet::new(py, python_node_names)?.into_any(),
            ),
        ],
    )
}
