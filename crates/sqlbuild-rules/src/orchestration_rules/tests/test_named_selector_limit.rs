use serde_json::json;

use crate::engine::_helpers::memo::evaluate_rows;
use crate::models::{EvaluateRequest, RowsRequest};
use crate::orchestration_rules::tests::test_types::NamedSelectorLimitTestCase;

fn job(name: &str, select: &str) -> String {
    format!("name = \"{name}\"\ndescription = \"d\"\nselect = \"{select}\"\n")
}

fn sensor(name: &str, condition: &str) -> String {
    format!(
        "from sqlbuild.orchestration import node_event, sensor, source_new_data\n\nSELECTION = \"a\"\n{name} = sensor(name=\"{name}_sensor\", description=\"d\", when={condition}, run=\"orders_export_job\")\n"
    )
}

fn export_job() -> (&'static str, String) {
    (
        "orchestration/jobs/orders/export.toml",
        job("orders_export_job", "orders"),
    )
}

fn limit_findings(
    files: &[(&str, String)],
    config: serde_json::Value,
) -> Result<Vec<(String, u64, u64, String)>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    for (path, contents) in files {
        let file = directory.path().join(path);
        std::fs::create_dir_all(file.parent().expect("parent"))
            .map_err(|error| error.to_string())?;
        std::fs::write(&file, contents).map_err(|error| error.to_string())?;
    }
    let mut config = config;
    config["cache"] = json!({ "enabled": false });
    let request = EvaluateRequest {
        project_dir: directory.path().to_string_lossy().into_owned(),
        config: serde_json::from_value(config).map_err(|error| error.to_string())?,
        ..EvaluateRequest::default()
    };
    let evaluation = evaluate_rows(RowsRequest {
        request,
        memo: None,
        retain: false,
    })
    .map_err(|error| format!("{error:?}"))?;
    Ok(evaluation
        .faults
        .into_iter()
        .filter(|fault| fault.code == "SQBRORCHESTRATION101")
        .map(|fault| (fault.path, fault.line, fault.column, fault.message))
        .collect())
}

#[test]
fn given_job_and_sensor_selections_when_evaluating_then_more_named_nodes_than_the_limit_are_findings()
 {
    let selected = json!({ "select": ["SQBRORCHESTRATION101"] });
    let test_cases = [
        NamedSelectorLimitTestCase {
            description: "four bare names exceed the default limit of three",
            files: vec![
                export_job(),
                (
                    "orchestration/jobs/orders/nightly.toml",
                    job(
                        "orders_nightly_job",
                        "raw_storefront_orders partner_orders_file request_carrier_collection refund_review",
                    ),
                ),
            ],
            config: selected.clone(),
            expected: Ok(vec![(
                "orchestration/jobs/orders/nightly.toml",
                3,
                10,
                "job 'orders_nightly_job' names 4 nodes in its selection (limit 3)",
            )]),
        },
        NamedSelectorLimitTestCase {
            description: "three names, an intersection's name and typed names stay within the limit",
            files: vec![
                export_job(),
                (
                    "orchestration/jobs/orders/nightly.toml",
                    job(
                        "orders_nightly_job",
                        "fct_orders,tag:orders task:refund_review source:raw_orders tag:finance",
                    ),
                ),
            ],
            config: selected.clone(),
            expected: Ok(vec![]),
        },
        NamedSelectorLimitTestCase {
            description: "tags, folders, expansions, globs, jobs and the same name twice add nothing",
            files: vec![
                export_job(),
                (
                    "orchestration/jobs/orders/nightly.toml",
                    job(
                        "orders_nightly_job",
                        "a b c a tag:orders models/orders python/orders +fct_orders fct_refunds+ fct_* job:orders_export_job orders_export_job",
                    ),
                ),
            ],
            config: selected.clone(),
            expected: Ok(vec![]),
        },
        NamedSelectorLimitTestCase {
            description: "the option lowers the limit",
            files: vec![
                export_job(),
                (
                    "orchestration/jobs/orders/nightly.toml",
                    job("orders_nightly_job", "fct_orders fct_refunds"),
                ),
            ],
            config: json!({
                "select": ["SQBRORCHESTRATION101"],
                "rule_options": { "SQBRORCHESTRATION101": { "max_named_selectors": 1 } },
            }),
            expected: Ok(vec![(
                "orchestration/jobs/orders/nightly.toml",
                3,
                10,
                "job 'orders_nightly_job' names 2 nodes in its selection (limit 1)",
            )]),
        },
        NamedSelectorLimitTestCase {
            description: "a sensor condition is counted, and one that is not a literal is reported",
            files: vec![
                export_job(),
                (
                    "orchestration/sensors/orders/watch.py",
                    sensor("orders_watch", "node_event(select=\"a b c d\", kinds=[])"),
                ),
                (
                    "orchestration/sensors/orders/fresh.py",
                    sensor("orders_fresh", "source_new_data(select=SELECTION)"),
                ),
            ],
            config: selected.clone(),
            expected: Ok(vec![
                (
                    "orchestration/sensors/orders/fresh.py",
                    4,
                    96,
                    "sensor 'orders_fresh_sensor' has a condition select that is not a string literal, so the nodes it names cannot be counted",
                ),
                (
                    "orchestration/sensors/orders/watch.py",
                    4,
                    91,
                    "sensor 'orders_watch_sensor' names 4 nodes in its selection (limit 3)",
                ),
            ]),
        },
        NamedSelectorLimitTestCase {
            description: "an exact rule exception for the file removes its finding",
            files: vec![
                export_job(),
                (
                    "orchestration/jobs/orders/nightly.toml",
                    job("orders_nightly_job", "a b c d"),
                ),
            ],
            config: json!({
                "select": ["SQBRORCHESTRATION101"],
                "rule_exceptions": [{
                    "rule": "SQBRORCHESTRATION101",
                    "path": "orchestration/jobs/orders/nightly.toml",
                    "reason": "A repair job for four specific models.",
                }],
            }),
            expected: Ok(vec![]),
        },
        NamedSelectorLimitTestCase {
            description: "without the rule selected, the declarations are not read",
            files: vec![(
                "orchestration/jobs/orders/nightly.toml",
                "not = [valid".to_owned(),
            )],
            config: json!({ "select": ["SQBRMODEL101"] }),
            expected: Ok(vec![]),
        },
    ];
    for test_case in test_cases {
        let findings = limit_findings(&test_case.files, test_case.config.clone());
        let expected = test_case.expected.map(|faults| {
            faults
                .into_iter()
                .map(|(path, line, column, message)| {
                    (path.to_owned(), line, column, message.to_owned())
                })
                .collect()
        });

        assert_eq!(
            findings,
            expected.map_err(str::to_owned),
            "{}",
            test_case.description
        );
    }
}
