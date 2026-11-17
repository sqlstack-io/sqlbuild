//! The project's lineage edges and selector indexes, from its native graph.

use crate::graph::_helpers::project::views::graph_views;
use crate::graph::models::NativeProjectGraph;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyRef, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The project's lineage edges and selector indexes, from its native graph.
#[pyfunction]
pub fn project_lineage_views<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let graph: PyRef<'py, NativeProjectGraph> =
        native_field(project, "lineage_graph")?.extract()?;
    graph_views(py, graph.graph())
}
