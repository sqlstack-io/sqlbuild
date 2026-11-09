use crate::graph::errors::SelectorError;
use crate::python_selection::_helpers::nodes::{names_job, select_nodes};
use crate::python_selection::constants::IGNORED_EXCLUDE_CODES;
use crate::python_selection::models::{NodeIndex, PythonSelectionGraph};

/// The Python node names exclude tokens name, skipping tokens that match nothing; sorted.
pub fn resolve_python_node_excludes(
    graph: &PythonSelectionGraph,
    exclude: &[String],
) -> Result<Vec<String>, SelectorError> {
    let index: NodeIndex<'_> = NodeIndex::new(graph);
    let mut excluded: Vec<String> = Vec::new();
    for token in exclude.iter().flat_map(|group| group.split_whitespace()) {
        if names_job(token) {
            continue;
        }
        match select_nodes(&index, &[token.to_owned()], &[]) {
            Ok(names) => excluded.extend(names),
            Err(error) if IGNORED_EXCLUDE_CODES.contains(&error.code) => {}
            Err(error) => return Err(error),
        }
    }
    excluded.sort();
    excluded.dedup();
    Ok(excluded)
}
