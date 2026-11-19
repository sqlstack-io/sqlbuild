use std::collections::BTreeSet;

/// The node settings the planner applies, which the table's node settings must equal.
pub(super) struct NodeSettingConsumersTestCase {
    pub(super) description: &'static str,
    pub(super) expected_node_settings: BTreeSet<&'static str>,
}
