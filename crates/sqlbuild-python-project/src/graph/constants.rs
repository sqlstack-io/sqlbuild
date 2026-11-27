//! Public constants of graph.

use crate::graph::_helpers::project::getters::read_compiled_project_graph;
use crate::graph::models::CompiledProjectGraph;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The Python node kind whose functions load sources.
pub(crate) const LOADER_KIND: &str = "loader";
/// The resource type of a load's graph key.
pub(crate) const LOAD_RESOURCE: &str = "load";

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[GetterEntry {
    class: class_of::<CompiledProjectGraph>,
    read: read_compiled_project_graph,
}];
