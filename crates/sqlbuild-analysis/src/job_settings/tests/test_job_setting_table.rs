use std::collections::BTreeSet;

use serde_json::json;

use crate::job_settings::constants::JOB_SETTINGS;
use crate::job_settings::main::conflict_help::conflict_help;
use crate::job_settings::models::SettingScope;
use crate::job_settings::tests::helpers::{
    conflicts_on_a_node, consumer_faults, resolves_build_wide, whole_run_value,
};
use crate::job_settings::tests::test_types::{
    ConflictHelpTestCase, JobSettingScopeTestCase, NodeConsumerTestCase, WholeRunTestCase,
};

#[test]
fn given_every_job_setting_when_named_jobs_merge_it_then_its_scope_decides_and_the_command_line_wins()
 {
    let test_cases = [
        JobSettingScopeTestCase {
            description: "a build-wide setting must agree, unset included, and applies to all",
            scope: SettingScope::Build,
            expected_build_conflict: true,
            expected_unset_conflict: true,
            expected_build_value: true,
            expected_node_conflict: false,
        },
        JobSettingScopeTestCase {
            description: "a node setting stays per node and conflicts on a shared node",
            scope: SettingScope::Node,
            expected_build_conflict: false,
            expected_unset_conflict: false,
            expected_build_value: false,
            expected_node_conflict: true,
        },
        JobSettingScopeTestCase {
            description: "a job limit stays with each job",
            scope: SettingScope::JobLimit,
            expected_build_conflict: false,
            expected_unset_conflict: false,
            expected_build_value: false,
            expected_node_conflict: false,
        },
    ];
    let job_scopes: BTreeSet<String> = JOB_SETTINGS
        .iter()
        .filter(|spec| spec.scope != SettingScope::RunRequest)
        .map(|spec| format!("{:?}", spec.scope))
        .collect();
    let case_scopes: BTreeSet<String> = test_cases
        .iter()
        .map(|case| format!("{:?}", case.scope))
        .collect();
    assert_eq!(
        job_scopes, case_scopes,
        "every job setting scope has a case"
    );
    for test_case in test_cases {
        for spec in JOB_SETTINGS
            .iter()
            .filter(|spec| spec.scope == test_case.scope)
        {
            assert_eq!(
                (
                    resolves_build_wide(spec.name, &["a_job", "b_job"], &[]).0,
                    resolves_build_wide(spec.name, &["a_job", "c_job"], &[]).0,
                    resolves_build_wide(spec.name, &["a_job"], &[]).1,
                    conflicts_on_a_node(spec.name),
                    resolves_build_wide(spec.name, &["a_job", "b_job"], &[spec.name]),
                ),
                (
                    test_case.expected_build_conflict,
                    test_case.expected_unset_conflict,
                    test_case.expected_build_value,
                    test_case.expected_node_conflict,
                    (false, false),
                ),
                "{}: {}",
                test_case.description,
                spec.name
            );
        }
    }
}

#[test]
fn given_a_whole_run_setting_when_named_jobs_share_its_value_then_it_is_the_build_s() {
    let test_cases = [
        WholeRunTestCase {
            description: "every named job sets one value",
            values: [Some(json!("ONE")), Some(json!("ONE")), None],
            expected_value: Some(json!("ONE")),
        },
        WholeRunTestCase {
            description: "the named jobs set different values",
            values: [Some(json!("ONE")), Some(json!("TWO")), None],
            expected_value: None,
        },
        WholeRunTestCase {
            description: "one named job leaves it unset",
            values: [Some(json!("ONE")), None, None],
            expected_value: None,
        },
    ];
    for test_case in test_cases {
        for spec in JOB_SETTINGS.iter().filter(|spec| spec.whole_run) {
            assert_eq!(
                whole_run_value(spec.name, test_case.values.clone()),
                test_case.expected_value,
                "{}: {}",
                test_case.description,
                spec.name
            );
        }
    }
}

#[test]
fn given_two_jobs_disagreeing_on_a_setting_when_reporting_then_the_help_offers_only_flags_that_settle_it()
 {
    let test_cases = [
        ConflictHelpTestCase {
            description: "--no-tests can only turn tests off",
            setting: "run_tests",
            expected_help: "make the jobs agree, set it in only one of them, pass --no-tests to make \
                            it false for all, or build with --no-job-settings",
        },
        ConflictHelpTestCase {
            description: "--full-refresh can only turn full refresh on",
            setting: "full_refresh",
            expected_help: "make the jobs agree, set it in only one of them, pass --full-refresh to \
                            make it true for all, or build with --no-job-settings",
        },
        ConflictHelpTestCase {
            description: "a build-wide flag cannot be left unset by one job",
            setting: "fail_fast",
            expected_help: "make the jobs agree, pass --fail-fast to make it true for all, or build \
                            with --no-job-settings",
        },
        ConflictHelpTestCase {
            description: "a flag pair gives either value",
            setting: "ignore_audit_failures",
            expected_help: "make the jobs agree, set it in only one of them, override it with \
                            --ignore-audit-failures or --no-ignore-audit-failures, or build with \
                            --no-job-settings",
        },
        ConflictHelpTestCase {
            description: "an option taking a value gives any",
            setting: "vars",
            expected_help: "make the jobs agree, override it with --vars, or build with \
                            --no-job-settings",
        },
        ConflictHelpTestCase {
            description: "no flag gives a retry table",
            setting: "[retry]",
            expected_help: "make the jobs agree, set [retry] in only one of them, or build with \
                            --no-job-settings",
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            conflict_help(test_case.setting),
            test_case.expected_help,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_the_job_setting_table_when_checked_then_each_node_setting_names_its_own_consumer() {
    let test_cases = [NodeConsumerTestCase {
        description: "a node setting without a consumer would never reach the plan",
        expected_unconsumed_node_settings: Vec::new(),
        expected_consumed_other_settings: Vec::new(),
        expected_shared_consumers: 0,
    }];
    for test_case in test_cases {
        assert_eq!(
            consumer_faults(),
            (
                test_case.expected_unconsumed_node_settings,
                test_case.expected_consumed_other_settings,
                test_case.expected_shared_consumers,
            ),
            "{}",
            test_case.description
        );
    }
}
