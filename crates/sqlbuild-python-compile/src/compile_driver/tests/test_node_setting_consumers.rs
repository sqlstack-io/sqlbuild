use std::collections::BTreeSet;

use sqlbuild_analysis::job_settings::constants::JOB_SETTINGS;
use sqlbuild_analysis::job_settings::models::{NodeConsumer, SettingScope};

use crate::compile_driver::_helpers::jobs::job_node_settings::node_setting;
use crate::compile_driver::tests::test_types::NodeSettingConsumersTestCase;

#[test]
fn given_every_node_setting_in_the_table_when_planning_then_a_consumer_applies_it() {
    let test_cases = [NodeSettingConsumersTestCase {
        description: "each node setting is read through the consumer its table row names",
        consumers: vec![
            NodeConsumer::RefreshOverrides,
            NodeConsumer::TestEntries,
            NodeConsumer::AuditEntries,
            NodeConsumer::NodeWarehouses,
        ],
        expected_node_settings: JOB_SETTINGS
            .iter()
            .filter(|spec| spec.scope == SettingScope::Node)
            .map(|spec| spec.name)
            .collect(),
    }];
    for test_case in test_cases {
        let read: BTreeSet<&str> = test_case.consumers.into_iter().map(node_setting).collect();
        assert_eq!(
            read, test_case.expected_node_settings,
            "{}",
            test_case.description
        );
    }
}
