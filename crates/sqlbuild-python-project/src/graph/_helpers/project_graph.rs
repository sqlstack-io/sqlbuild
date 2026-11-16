//! The compiled project graph and selector resolution, held natively for Python's graph users.

use crate::graph::models::CompiledProjectGraph;
use crate::graph::models::NativeProjectGraph;
use crate::graph::models::ResourceRow;
use sqlbuild_python_base::positional_record;

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;
use sqlbuild_analysis::graph::main::build_project_graph::build_project_graph;
use sqlbuild_analysis::graph::models::GraphResource;

use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

type Edges = Vec<(ObjectKey, Vec<ObjectKey>)>;
positional_record! {
    extract, into;
    /// `(names, upstream, downstream, tags, paths)` in Python's dict order.
    pub(crate) struct IndexesRow {
        pub(crate) names: Vec<(String, ObjectKey)>,
        pub(crate) upstream: Edges,
        pub(crate) downstream: Edges,
        pub(crate) tags: Vec<(String, Vec<ObjectKey>)>,
        pub(crate) paths: Vec<(ObjectKey, String)>,
    }
}
/// `(unique id, neighbour unique ids)` in graph order.
pub(crate) type UniqueIdMap = Vec<(String, Vec<String>)>;
/// `PlannerInputError`'s `(code, message, help)`.
type FailureRow = (&'static str, String, Option<String>);
/// The keys, or the `PlannerInputError` Python raises instead.
pub(crate) type KeysOutcome = (Option<Vec<ObjectKey>>, Option<FailureRow>);

pub(crate) fn dependency_edges(key: &ObjectKey, deps: &[ObjectKey]) -> Vec<(ObjectKey, ObjectKey)> {
    deps.iter().map(|dep| (dep.clone(), key.clone())).collect()
}

pub(crate) fn outcome(result: Result<Vec<ObjectKey>, SelectorError>) -> KeysOutcome {
    match result {
        Ok(keys) => (Some(keys), None),
        Err(error) => (None, Some(failure(error))),
    }
}

fn failure(error: SelectorError) -> FailureRow {
    (error.code, error.message, error.help)
}

/// The project graph indexes of these resources, in project order.
pub(crate) fn project_graph(resources: Vec<ResourceRow>) -> PyResult<NativeProjectGraph> {
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<NativeProjectGraph>()?;
    module.add_class::<CompiledProjectGraph>()
}
