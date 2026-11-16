//! The project's natively held graph; dict views are built only when a boundary reads them.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_project::graph::models::{CompiledProjectGraph, NativeProjectGraph};

/// The project's natively held graph; dict views are built only when a boundary reads them.
pub fn project_graph<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let native: Bound<'py, NativeProjectGraph> =
        native_field(project, "lineage_graph")?.cast_into()?;
    Ok(Bound::new(
        project.py(),
        CompiledProjectGraph::new(project.clone().unbind(), native.unbind()),
    )?
    .into_any())
}
