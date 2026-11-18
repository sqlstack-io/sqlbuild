use std::collections::BTreeMap;

use serde_json::json;

use crate::job_settings::main::build_job_settings::build_job_settings;
use crate::job_settings::main::group_setting_mismatch::group_setting_mismatch;
use crate::job_settings::main::named_jobs::named_jobs;
use crate::job_settings::tests::helpers::{declared, strings};
use crate::job_settings::tests::test_types::{BuildJobSettingsTestCase, GroupMismatchTestCase};

#[test]
fn given_named_jobs_when_merging_build_settings_then_they_must_agree() {
    let test_cases = [
        BuildJobSettingsTestCase {
            description: "a job that leaves vars unset differs from one that sets them",
            select: &["crm_job", "job:orders_job"],
            expected_named: vec!["crm_job", "orders_job"],
            expected: Err(
                "jobs crm_job and orders_job set vars differently ({\"region\":\"eu\"} \
                 and not set); a build applies vars to everything it runs",
            ),
        },
        BuildJobSettingsTestCase {
            description: "a single job's vars",
            select: &["job:crm_job,customers"],
            expected_named: vec!["crm_job"],
            expected: Ok(BTreeMap::from([("vars", json!({"region": "eu"}))])),
        },
        BuildJobSettingsTestCase {
            description: "two jobs with different vars",
            select: &["job:crm_job job:crm_us_job"],
            expected_named: vec!["crm_job", "crm_us_job"],
            expected: Err(
                "jobs crm_job and crm_us_job set vars differently ({\"region\":\"eu\"} \
                 and {\"region\":\"us\"}); a build applies vars to everything it runs",
            ),
        },
        BuildJobSettingsTestCase {
            description: "a group's jobs without build-wide settings",
            select: &["job:daily_group"],
            expected_named: vec!["orders_job", "refunds_job"],
            expected: Ok(BTreeMap::new()),
        },
        BuildJobSettingsTestCase {
            description: "tags, paths and node names name no job",
            select: &["tag:daily models/marts orders"],
            expected_named: vec![],
            expected: Ok(BTreeMap::new()),
        },
    ];
    for test_case in test_cases {
        let named: Vec<String> = named_jobs(&strings(test_case.select), &declared());
        assert_eq!(named, test_case.expected_named, "{}", test_case.description);
        let merged = build_job_settings(&named, &declared());
        let merged = merged
            .map(|settings| settings.into_iter().collect::<BTreeMap<String, _>>())
            .map_err(|conflict| conflict.message);
        let expected = test_case
            .expected
            .map(|settings| {
                settings
                    .into_iter()
                    .map(|(key, value)| (key.to_owned(), value))
                    .collect()
            })
            .map_err(str::to_owned);
        assert_eq!(merged, expected, "{}", test_case.description);
    }
}

#[test]
fn given_a_group_when_its_jobs_differ_on_vars_or_changed_then_it_cannot_run_as_one_build() {
    let test_cases = [
        GroupMismatchTestCase {
            description: "different vars",
            members: &["crm_job", "crm_us_job"],
            expected: Some(
                "jobs crm_job and crm_us_job set vars differently; a group runs its \
                 jobs as one build, which has one vars",
            ),
        },
        GroupMismatchTestCase {
            description: "vars set by only one job still differ",
            members: &["orders_job", "crm_job"],
            expected: Some(
                "jobs orders_job and crm_job set vars differently; a group runs its \
                 jobs as one build, which has one vars",
            ),
        },
        GroupMismatchTestCase {
            description: "node settings may differ",
            members: &["orders_job", "refunds_job", "quiet_job"],
            expected: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            group_setting_mismatch(&strings(test_case.members), &declared()).as_deref(),
            test_case.expected,
            "{}",
            test_case.description
        );
    }
}
