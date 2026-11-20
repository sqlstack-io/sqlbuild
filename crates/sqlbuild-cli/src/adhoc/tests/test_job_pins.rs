//! A build naming jobs pins each to the frozen nodes it gives settings to, as `sqb build` decides them.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sqlbuild_orchestration::runs::models::PublishedJobs;

use crate::adhoc::main::adhoc_planner::adhoc_planner;
use crate::adhoc::tests::helpers::{expected_pins, nightly_pin, resource, weekly_pin};
use crate::adhoc::tests::test_types::JobPinTestCase;

#[test]
fn given_a_build_naming_jobs_when_planned_then_each_is_pinned_to_the_nodes_it_gives_settings_to() {
    let resources = [
        resource("model:orders", &[], &["daily"], None),
        resource("model:order_totals", &["model:orders"], &["daily"], None),
        resource("model:customers", &[], &[], None),
        resource("model:nightly", &[], &["daily"], None),
    ];
    let keys =
        |keys: &[&str]| -> BTreeSet<String> { keys.iter().map(|key| (*key).to_owned()).collect() };
    let settings = |value: Value| -> BTreeMap<String, Value> {
        serde_json::from_value(value).expect("settings")
    };
    let jobs = PublishedJobs {
        selections: BTreeMap::from([
            (
                "nightly".to_owned(),
                keys(&["model:order_totals", "model:orders"]),
            ),
            ("weekly".to_owned(), keys(&["model:customers"])),
            ("south".to_owned(), keys(&["model:customers"])),
            ("keep_orders".to_owned(), keys(&["model:orders"])),
            (
                "reports".to_owned(),
                keys(&["model:customers", "model:order_totals", "model:orders"]),
            ),
        ]),
        groups: BTreeMap::from([(
            "reports".to_owned(),
            vec!["nightly".to_owned(), "weekly".to_owned()],
        )]),
        settings: BTreeMap::from([
            (
                "nightly".to_owned(),
                settings(
                    json!({"concurrency": 1, "full_refresh": true, "vars": {"region": "north"}}),
                ),
            ),
            (
                "weekly".to_owned(),
                settings(json!({"vars": {"region": "north"}, "warehouse": "REPORTS_XS"})),
            ),
            (
                "south".to_owned(),
                settings(json!({"vars": {"region": "south"}})),
            ),
            (
                "keep_orders".to_owned(),
                settings(json!({"full_refresh": false, "vars": {"region": "north"}})),
            ),
        ]),
    };
    let north = json!({"vars": {"region": "north"}});
    let test_cases = [
        JobPinTestCase {
            description: "a named job pins its nodes and gives its build-wide vars",
            command: &["build", "--select", "job:nightly"],
            expected_plan: Ok((
                &["model:order_totals", "model:orders"],
                json!({"nightly": nightly_pin(&["model:order_totals", "model:orders"])}),
                north.clone(),
            )),
        },
        JobPinTestCase {
            description: "an intersection pins only the nodes its token selects",
            command: &["build", "--select", "job:nightly,orders"],
            expected_plan: Ok((
                &["model:orders"],
                json!({"nightly": nightly_pin(&["model:orders"])}),
                north.clone(),
            )),
        },
        JobPinTestCase {
            description: "an excluded node is not pinned",
            command: &[
                "build",
                "--select",
                "job:nightly",
                "--exclude",
                "order_totals",
            ],
            expected_plan: Ok((
                &["model:orders"],
                json!({"nightly": nightly_pin(&["model:orders"])}),
                north.clone(),
            )),
        },
        JobPinTestCase {
            description: "a group pins each of its jobs",
            command: &["build", "--select", "job:reports"],
            expected_plan: Ok((
                &["model:customers", "model:order_totals", "model:orders"],
                json!({
                    "nightly": nightly_pin(&["model:order_totals", "model:orders"]),
                    "weekly": weekly_pin(&["model:customers"]),
                }),
                north.clone(),
            )),
        },
        JobPinTestCase {
            description: "a named job left with none of its nodes stays pinned for its build-wide settings, its warehouse among them",
            command: &[
                "build",
                "--select",
                "job:weekly",
                "orders",
                "--exclude",
                "customers",
            ],
            expected_plan: Ok((
                &["model:orders"],
                json!({"weekly": weekly_pin(&[])}),
                json!({"vars": {"region": "north"}, "warehouse": "REPORTS_XS"}),
            )),
        },
        JobPinTestCase {
            description: "typed vars replace the jobs' vars",
            command: &[
                "build",
                "--select",
                "job:nightly",
                "--vars",
                "{\"region\": \"east\"}",
            ],
            expected_plan: Ok((
                &["model:order_totals", "model:orders"],
                json!({"nightly": nightly_pin(&["model:order_totals", "model:orders"])}),
                json!({}),
            )),
        },
        JobPinTestCase {
            description: "a tag selecting the model named like a job pins no job",
            command: &["build", "--select", "tag:daily"],
            expected_plan: Ok((
                &["model:nightly", "model:order_totals", "model:orders"],
                json!({}),
                json!({}),
            )),
        },
        JobPinTestCase {
            description: "a test run takes no job settings",
            command: &["test", "--select", "job:nightly"],
            expected_plan: Ok((
                &["model:order_totals", "model:orders"],
                json!({}),
                json!({}),
            )),
        },
        JobPinTestCase {
            description: "named jobs that set vars differently are refused, as the build would",
            command: &["build", "--select", "job:nightly", "job:south"],
            expected_plan: Err("set vars differently"),
        },
        JobPinTestCase {
            description: "named jobs that give one node different values are refused with S027, as the build would",
            command: &["build", "--select", "job:nightly", "job:keep_orders"],
            expected_plan: Err("S027: "),
        },
    ];
    let publication = adhoc_planner().prepare(resources.to_vec(), jobs);
    for test_case in test_cases {
        let command: Vec<String> = test_case
            .command
            .iter()
            .map(|token| (*token).to_owned())
            .collect();
        let planned = publication
            .plan(&command)
            .map(|plan| {
                (
                    plan.selection.into_iter().collect::<Vec<String>>(),
                    json!(plan.jobs),
                    json!(plan.job_settings),
                )
            })
            .map_err(|refusal| {
                test_case
                    .expected_plan
                    .as_ref()
                    .err()
                    .filter(|fragment| refusal.contains(**fragment))
                    .map_or(refusal.clone(), |fragment| (*fragment).to_owned())
            });
        assert_eq!(
            planned,
            expected_pins(test_case.expected_plan),
            "{}",
            test_case.description
        );
    }
}
