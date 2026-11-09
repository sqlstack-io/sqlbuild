use crate::graph::models::ProjectGraph;

/// Whether `selectors` name a job, either by `job:` or as a bare name the graph and `also_known` lack.
pub fn selectors_need_jobs(
    graph: &ProjectGraph,
    selectors: &[String],
    also_known: &[&str],
) -> bool {
    crate::graph::_helpers::selectors::needs_jobs(graph, selectors, also_known)
}
