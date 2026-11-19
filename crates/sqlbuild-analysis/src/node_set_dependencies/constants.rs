//! Kinds, codes and messages of job and tag dependencies.

/// A dependency on a job's (or job group's) selection.
pub(crate) const JOB_SET: &str = "job";
/// A dependency on every node carrying a tag.
pub(crate) const TAG_SET: &str = "tag";
/// The planner code these compile errors carry.
pub(crate) const NODE_SET_CODE: &str = "S000";
/// SQL resource types a Python node may depend on.
pub(crate) const DEPENDABLE_SQL_KINDS: [&str; 3] = ["model", "seed", "source"];
/// Python node kinds a Python node may depend on.
pub(crate) const DEPENDABLE_PYTHON_KINDS: [&str; 2] = ["loader", "asset"];
/// The Python node kind that may depend on Python nodes only.
pub(crate) const LOADER_KIND: &str = "loader";
/// The SQL resource type a loader fills.
pub(crate) const SOURCE_KIND: &str = "source";
/// What separates a code location from a job name in a cross-location dependency.
pub(crate) const LOCATION_SEPARATOR: char = ':';
/// Nodes a cycle error names one by one before summarising the rest.
pub(crate) const NAMED_NODES: usize = 5;
/// The identity metadata key a Python node's resolved set dependencies are recorded under.
pub(crate) const SET_DEPENDENCIES_KEY: &str = "set_dependencies";
/// Each recorded set's label key.
pub(crate) const SET_KEY: &str = "set";
/// Each recorded set's end node labels key.
pub(crate) const ENDS_KEY: &str = "ends";
