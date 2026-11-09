use std::collections::BTreeMap;

use crate::graph::models::{JobSelection, ProjectGraph};

/// Attach jobs and job groups so `job:<name>` and a bare job name select what each job selects.
pub fn attach_jobs(graph: ProjectGraph, catalog: BTreeMap<String, JobSelection>) -> ProjectGraph {
    crate::graph::_helpers::jobs::attach(graph, catalog)
}
