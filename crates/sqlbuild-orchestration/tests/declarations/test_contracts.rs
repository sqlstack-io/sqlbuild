//! Job group wait contracts, derived from node lineage without a control store.

#[path = "helpers.rs"]
pub(crate) mod helpers;

#[path = "test_group_wait_graph.rs"]
mod test_group_wait_graph;

#[path = "test_types.rs"]
pub(crate) mod test_types;

#[path = "test_group_dependencies.rs"]
mod test_group_dependencies;
