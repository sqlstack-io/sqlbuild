use sqlbuild_analysis::job_settings::models::SettingScope;

use crate::argv::tests::helpers::{parser_options, table_options, unrendered_settings};
use crate::argv::tests::test_types::{JobSettingTableTestCase, RenderedSettingTestCase};

#[test]
fn given_the_build_parser_when_compared_with_the_job_setting_table_then_every_option_has_one_row() {
    let test_cases = [JobSettingTableTestCase {
        description: "add a JOB_SETTINGS row for each new build option",
        command: "build",
        expected_rows: table_options(),
    }];
    for test_case in test_cases {
        assert_eq!(
            parser_options(test_case.command),
            test_case.expected_rows,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_job_setting_the_build_takes_as_a_flag_when_rendered_then_it_is_one_of_its_options() {
    let test_cases = [
        RenderedSettingTestCase {
            description: "a build-wide setting travels as its own option",
            scope: SettingScope::Build,
            whole_run: false,
            expected_unrendered: Vec::new(),
        },
        RenderedSettingTestCase {
            description: "a whole-run node setting travels as its own option",
            scope: SettingScope::Node,
            whole_run: true,
            expected_unrendered: Vec::new(),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            unrendered_settings(test_case.scope, test_case.whole_run),
            test_case.expected_unrendered,
            "{}",
            test_case.description
        );
    }
}
