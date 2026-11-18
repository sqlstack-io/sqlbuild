use std::collections::BTreeMap;

use serde_json::json;

use crate::job_settings::errors::NodeJobSettingsError;
use crate::job_settings::main::node_job_settings::node_job_settings;
use crate::job_settings::models::NodeJobSettings;
use crate::job_settings::tests::helpers::{declared, graph_with_jobs, strings};
use crate::job_settings::tests::test_types::{NodeJobConflictTestCase, NodeJobSettingsTestCase};

fn settings(select: &[&str], exclude: &[&str]) -> Result<NodeJobSettings, NodeJobSettingsError> {
    node_job_settings(
        &graph_with_jobs(),
        (&strings(select), &strings(exclude)),
        &declared(),
    )
}

#[test]
fn given_named_jobs_when_resolving_node_settings_then_only_their_own_selected_nodes_take_them() {
    let orders_values =
        || BTreeMap::from([("run_tests", json!(false)), ("full_refresh", json!(true))]);
    let test_cases = [
        NodeJobSettingsTestCase {
            description: "a named job gives each of its nodes its settings and limit",
            select: &["job:orders_job"],
            exclude: &[],
            expected_node_jobs: BTreeMap::from([
                ("orders", vec!["orders_job"]),
                ("stg_orders", vec!["orders_job"]),
            ]),
            expected_values: BTreeMap::from([
                ("orders", orders_values()),
                ("stg_orders", orders_values()),
            ]),
            expected_job_limits: BTreeMap::from([("orders_job", 2)]),
        },
        NodeJobSettingsTestCase {
            description: "an intersection gives the job's settings to just the nodes it selects",
            select: &["job:orders_job,orders"],
            exclude: &[],
            expected_node_jobs: BTreeMap::from([("orders", vec!["orders_job"])]),
            expected_values: BTreeMap::from([("orders", orders_values())]),
            expected_job_limits: BTreeMap::from([("orders_job", 2)]),
        },
        NodeJobSettingsTestCase {
            description: "nodes added beside the job, or by + around it, take none of its settings",
            select: &["job:orders_job customers", "job:orders_job+"],
            exclude: &[],
            expected_node_jobs: BTreeMap::from([
                ("orders", vec!["orders_job"]),
                ("stg_orders", vec!["orders_job"]),
            ]),
            expected_values: BTreeMap::from([
                ("orders", orders_values()),
                ("stg_orders", orders_values()),
            ]),
            expected_job_limits: BTreeMap::from([("orders_job", 2)]),
        },
        NodeJobSettingsTestCase {
            description: "an excluded node takes nothing",
            select: &["orders_job"],
            exclude: &["stg_orders"],
            expected_node_jobs: BTreeMap::from([("orders", vec!["orders_job"])]),
            expected_values: BTreeMap::from([("orders", orders_values())]),
            expected_job_limits: BTreeMap::from([("orders_job", 2)]),
        },
        NodeJobSettingsTestCase {
            description: "naming a group names its jobs; a shared node keeps both jobs' limits",
            select: &["job:daily_group"],
            exclude: &[],
            expected_node_jobs: BTreeMap::from([
                ("orders", vec!["orders_job"]),
                ("refunds", vec!["refunds_job"]),
                ("stg_orders", vec!["orders_job", "refunds_job"]),
            ]),
            expected_values: BTreeMap::from([
                ("orders", orders_values()),
                ("stg_orders", orders_values()),
            ]),
            expected_job_limits: BTreeMap::from([("orders_job", 2), ("refunds_job", 4)]),
        },
        NodeJobSettingsTestCase {
            description: "jobs that agree, or where only one sets a setting, do not conflict",
            select: &["job:quiet_job job:orders_job"],
            exclude: &[],
            expected_node_jobs: BTreeMap::from([
                ("orders", vec!["orders_job"]),
                ("stg_orders", vec!["quiet_job", "orders_job"]),
            ]),
            expected_values: BTreeMap::from([
                ("orders", orders_values()),
                ("stg_orders", orders_values()),
            ]),
            expected_job_limits: BTreeMap::from([("orders_job", 2)]),
        },
        NodeJobSettingsTestCase {
            description: "a selection that names no job takes no settings",
            select: &["+orders"],
            exclude: &[],
            expected_node_jobs: BTreeMap::new(),
            expected_values: BTreeMap::new(),
            expected_job_limits: BTreeMap::new(),
        },
    ];
    for test_case in test_cases {
        let resolved: NodeJobSettings =
            settings(test_case.select, test_case.exclude).expect(test_case.description);
        let node_jobs: BTreeMap<&str, Vec<&str>> = resolved
            .node_jobs
            .iter()
            .map(|(key, jobs)| (key.1.as_str(), jobs.iter().map(String::as_str).collect()))
            .collect();
        let values: BTreeMap<&str, BTreeMap<&str, serde_json::Value>> = resolved
            .values
            .iter()
            .map(|(key, values)| {
                (
                    key.1.as_str(),
                    values
                        .iter()
                        .map(|(name, value)| (name.as_str(), value.clone()))
                        .collect(),
                )
            })
            .collect();
        let limits: BTreeMap<&str, i64> = resolved
            .job_limits
            .iter()
            .map(|(job, limit)| (job.as_str(), *limit))
            .collect();
        assert_eq!(
            node_jobs, test_case.expected_node_jobs,
            "{}",
            test_case.description
        );
        assert_eq!(
            values, test_case.expected_values,
            "{}",
            test_case.description
        );
        assert_eq!(
            limits, test_case.expected_job_limits,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_two_named_jobs_setting_one_node_differently_when_resolving_then_it_is_a_conflict() {
    let test_cases = [
        NodeJobConflictTestCase {
            description: "a node-level setting",
            select: &["job:staging_job", "job:orders_job"],
            expected_message: "model:stg_orders is selected through jobs staging_job and orders_job, \
                 which set run_tests differently (true and false)",
        },
        NodeJobConflictTestCase {
            description: "a retry policy",
            select: &["job:retried_job job:retried_fast_job"],
            expected_message: "model:stg_orders is selected through jobs retried_job and \
                 retried_fast_job, which set [retry] differently (3 attempts, 60s apart and 2 \
                 attempts, 0s apart)",
        },
    ];
    for test_case in test_cases {
        let Err(NodeJobSettingsError::Conflict(conflict)) = settings(test_case.select, &[]) else {
            panic!("{}: expected a conflict", test_case.description);
        };
        assert_eq!(
            conflict.message, test_case.expected_message,
            "{}",
            test_case.description
        );
    }
}
