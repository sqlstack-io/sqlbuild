//! Project graph.

use crate::compiled_views::main::project::sources::sources;
use crate::planner_scope::_helpers::values::loader_graph;
use crate::resource_views::main::source_reads::source_entry::source_entry;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::planner_scope::models::LoaderGraph;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Project graph.
pub fn project_graph(project: &Bound<'_, PyAny>) -> PyResult<LoaderGraph> {
    let mut graph = loader_graph(&native_field(project, "loader_functions")?)?;
    for source in sources(project)?.try_iter()? {
        let entry = source_entry(&source?)?;
        graph.sources.push((
            native_field(&entry, "name")?.extract()?,
            native_field(&entry, "loader")?.extract()?,
        ));
    }
    Ok(graph)
}
