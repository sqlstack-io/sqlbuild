use std::collections::BTreeSet;

use sqlbuild_analysis::job_settings::constants::{JOB_SETTINGS, WAREHOUSE_SETTING};
use sqlbuild_analysis::job_settings::models::SettingScope;

use crate::compile_driver::_helpers::jobs::job_node_settings::{
    FULL_REFRESH, IGNORE_AUDIT_FAILURES, RUN_TESTS,
};
use crate::compile_driver::tests::test_types::NodeSettingConsumersTestCase;

#[test]
fn given_every_node_setting_in_the_table_when_planning_then_a_consumer_applies_it() {
    let test_cases = [NodeSettingConsumersTestCase {
        description: "apply each node setting to the plan in job_node_settings.rs, then list it here",
        expected_node_settings: BTreeSet::from([
            FULL_REFRESH,
            RUN_TESTS,
            IGNORE_AUDIT_FAILURES,
            WAREHOUSE_SETTING,
        ]),
    }];
    for test_case in test_cases {
        let node_settings: BTreeSet<&str> = JOB_SETTINGS
            .iter()
            .filter(|spec| spec.scope == SettingScope::Node)
            .map(|spec| spec.name)
            .collect();
        assert_eq!(
            node_settings, test_case.expected_node_settings,
            "{}",
            test_case.description
        );
    }
}
