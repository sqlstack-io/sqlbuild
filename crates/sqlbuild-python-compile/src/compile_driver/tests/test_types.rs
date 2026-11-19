use std::collections::BTreeSet;

use sqlbuild_analysis::job_settings::models::NodeConsumer;

/// The consumers the planner reads node settings through, and the settings they must name.
pub(super) struct NodeSettingConsumersTestCase {
    pub(super) description: &'static str,
    pub(super) consumers: Vec<NodeConsumer>,
    pub(super) expected_node_settings: BTreeSet<&'static str>,
}

/// A singular audit's read nodes' job values, the build's value, and whether it ignores failures.
pub(super) struct SingularAuditIgnoresTestCase {
    pub(super) description: &'static str,
    pub(super) job_values: Vec<Option<bool>>,
    pub(super) build_ignores: bool,
    pub(super) expected_ignores: Option<bool>,
}
