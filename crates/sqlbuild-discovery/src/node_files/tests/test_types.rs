use crate::node_files::models::NodeModule;

/// Modules and the problem the file-name check reports for them.
pub(super) struct NodeFilesTestCase {
    pub(super) description: &'static str,
    pub(super) modules: Vec<NodeModule>,
    pub(super) expected_problem: Option<&'static str>,
}
