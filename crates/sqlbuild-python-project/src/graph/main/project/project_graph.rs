//! The project graph indexes of these resources, in project order.

use crate::graph::models::NativeProjectGraph;
use crate::graph::models::ResourceRow;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::graph::main::build_project_graph::build_project_graph;
use sqlbuild_analysis::graph::models::GraphResource;
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

/// The project graph indexes of these resources, in project order.
pub fn project_graph(resources: Vec<ResourceRow>) -> PyResult<NativeProjectGraph> {
    compiler_guard(|| {
        let resources: Vec<GraphResource> = resources
            .into_iter()
            .map(
                |ResourceRow {
                     key,
                     deps,
                     tags,
                     folder,
                 }| GraphResource {
                    key,
                    deps,
                    tags,
                    folder,
                },
            )
            .collect();
        Ok(NativeProjectGraph::new(build_project_graph(&resources)))
    })
}
