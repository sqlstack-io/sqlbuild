//! `unknown_tags`: each dependant's `tag(...)` dependencies that no node carries.

use crate::graph::models::ProjectGraph;
use crate::node_set_dependencies::_helpers::sets::tag_carried;
use crate::node_set_dependencies::constants::TAG_SET;
use crate::node_set_dependencies::models::DependantNode;
use crate::python_selection::models::PythonSelectionGraph;

/// Each `(dependant, tag)` where the dependant depends on a tag no SQL resource or Python node carries.
pub fn unknown_tags(
    graph: &ProjectGraph,
    python: &PythonSelectionGraph,
    dependants: &[DependantNode],
) -> Vec<(String, String)> {
    let mut unknown: Vec<(String, String)> = Vec::new();
    for dependant in dependants {
        for dependency in &dependant.sets {
            if dependency.kind == TAG_SET && !tag_carried(graph, python, &dependency.name) {
                unknown.push((dependant.name.clone(), dependency.name.clone()));
            }
        }
    }
    unknown
}
