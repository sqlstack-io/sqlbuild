//! Node module fixtures for the file-name check.

use crate::node_files::models::NodeModule;

/// A module at `path` with its direct declarations and its factories' kinds.
pub(super) fn module(
    path: &str,
    direct: &[(&str, &'static str)],
    factories: &[(&str, &[&'static str])],
) -> NodeModule {
    NodeModule {
        relative_path: path.to_owned(),
        direct: direct
            .iter()
            .map(|(name, kind)| ((*name).to_owned(), *kind))
            .collect(),
        factories: factories
            .iter()
            .map(|(name, kinds)| ((*name).to_owned(), kinds.to_vec()))
            .collect(),
    }
}
