//! Graph walks over the natively hosted Python-node graph record.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{
    PyDict, PyDictMethods, PyFrozenSet, PySet, PySetMethods, PyTuple, PyTupleMethods,
};
use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;

use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_project::graph::models::PyNodeSet;

/// `PythonNodeGraph.selected_sql_refs(selected_names=...)`: SQL references of the selected closure.
pub(crate) fn graph_selected_sql_refs(
    graph: &Bound<'_, PyAny>,
    args: &Bound<'_, PyTuple>,
    kwargs: Option<&Bound<'_, PyDict>>,
) -> PyResult<Py<PyAny>> {
    let py = graph.py();
    let keyword: Option<Bound<'_, PyAny>> = match kwargs {
        Some(kwargs) => kwargs.get_item("selected_names")?,
        None => None,
    };
    let selected: Bound<'_, PyAny> = match keyword {
        Some(selected) => selected,
        None => args.get_item(0)?,
    };
    let nodes_by_name: Bound<'_, PyAny> = native_field(graph, "nodes_by_name")?;
    let upstream: Bound<'_, PyAny> = native_field(graph, "upstream_deps")?;
    let refs: Bound<'_, PySet> = PySet::empty(py)?;
    let starts: Vec<Bound<'_, PyAny>> = selected.try_iter()?.collect::<PyResult<_>>()?;
    let mut visited: PyNodeSet<'_> = PyNodeSet(PySet::new(py, &starts)?);
    breadth_first_reach(starts, None, &mut visited, |name| {
        mapping_get_or(&upstream, name, PyTuple::empty(py))?
            .try_iter()?
            .collect::<PyResult<Vec<Bound<'_, PyAny>>>>()
    })?;
    for name in visited.0.iter() {
        let node: Bound<'_, PyAny> = mapping_get(&nodes_by_name, name)?;
        if !node.is_none() {
            for reference in native_field(&node, "sql_deps")?.try_iter()? {
                refs.add(reference?)?;
            }
        }
    }
    Ok(PyFrozenSet::new(py, refs.iter())?.into_any().unbind())
}
