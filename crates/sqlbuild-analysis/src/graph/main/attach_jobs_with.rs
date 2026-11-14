use std::collections::{BTreeMap, BTreeSet};

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;
use crate::graph::models::{JobSelection, ProjectGraph};

/// [`super::attach_jobs::attach_jobs`] with each job's selectors resolved by `resolve_job`.
pub fn attach_jobs_with(
    graph: ProjectGraph,
    catalog: BTreeMap<String, JobSelection>,
    resolve_job: impl Fn(
        &ProjectGraph,
        &[String],
        &[String],
    ) -> Result<BTreeSet<ObjectKey>, SelectorError>,
) -> ProjectGraph {
    crate::graph::_helpers::jobs::attach_with(graph, catalog, resolve_job)
}
