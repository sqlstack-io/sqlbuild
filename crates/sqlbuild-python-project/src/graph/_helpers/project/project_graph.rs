//! The compiled project graph and selector resolution, held natively for Python's graph users.

use sqlbuild_python_base::positional_record;

use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_analysis::graph::errors::SelectorError;

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
