use crate::tests::helpers::install_test_identity;
use serde_json::json;

use crate::orchestration_rules::tests::helpers::{
    export_job, job, limit_findings, owned_findings, sensor,
};
use crate::orchestration_rules::tests::test_types::NamedSelectorLimitTestCase;

#[test]
fn given_job_and_sensor_selections_when_evaluating_then_more_named_nodes_than_the_limit_are_findings()
 {
    install_test_identity();
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
            expected_findings: vec![(
                "orchestration/jobs/orders/nightly.toml",
                3,
                10,
                "job 'orders_nightly_job' names 4 nodes in its selection (limit 3)",
            )],
        },
        NamedSelectorLimitTestCase {
            description: "three names, an intersection's name and typed names stay within the limit",
            files: vec![
                export_job(),
                (
                    "orchestration/jobs/orders/nightly.toml",
                    job(
                        "orders_nightly_job",
                        "fct_orders,tag:orders asset:refund_review source:raw_orders tag:finance",
                    ),
                ),
            ],
            config: selected.clone(),
            expected_findings: vec![],
        },
        NamedSelectorLimitTestCase {
            description: "tags, folders, expansions, globs, jobs and the same name twice add nothing",
            files: vec![
                export_job(),
                (
                    "orchestration/jobs/orders/nightly.toml",
                    job(
                        "orders_nightly_job",
                        "a b c a tag:orders sql/models/orders python/nodes/orders +fct_orders fct_refunds+ fct_* job:orders_export_job orders_export_job",
                    ),
                ),
            ],
            config: selected.clone(),
            expected_findings: vec![],
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
            expected_findings: vec![(
                "orchestration/jobs/orders/nightly.toml",
                3,
                10,
                "job 'orders_nightly_job' names 2 nodes in its selection (limit 1)",
            )],
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
            expected_findings: vec![
                (
                    "orchestration/sensors/orders/fresh.py",
                    4,
                    54,
                    "sensor 'orders_fresh_sensor' has a condition select that is not a string literal, so the nodes it names cannot be counted",
                ),
                (
                    "orchestration/sensors/orders/watch.py",
                    4,
                    49,
                    "sensor 'orders_watch_sensor' names 4 nodes in its selection (limit 3)",
                ),
            ],
        },
        NamedSelectorLimitTestCase {
            description: "imports of project names are followed, platform events are skipped, and what cannot be resolved is reported",
            files: vec![
                export_job(),
                (
                    "orchestration/sensors/orders/_conditions.py",
                    "from sqlbuild.orchestration import node_event, platform_event\nLATE = node_event(select=\"a b c d\", kinds=[])\nTICK = platform_event(kinds=[])\n".to_owned(),
                ),
                (
                    "orchestration/sensors/orders/late.py",
                    format!(
                        "from orchestration.sensors.orders._conditions import LATE\n{}",
                        sensor("orders_late", "LATE")
                    ),
                ),
                (
                    "orchestration/sensors/orders/tick.py",
                    format!("from ._conditions import TICK\n{}", sensor("orders_tick", "TICK")),
                ),
                (
                    "orchestration/sensors/orders/built.py",
                    sensor("orders_built", "make_condition(\"orders\")"),
                ),
                (
                    "orchestration/sensors/orders/shared.py",
                    format!(
                        "from shared_conditions import READY\n{}",
                        sensor("orders_shared", "READY")
                    ),
                ),
            ],
            config: selected.clone(),
            expected_findings: vec![
                (
                    "orchestration/sensors/orders/built.py",
                    4,
                    31,
                    "sensor 'orders_built_sensor' has a condition the rule cannot resolve: `make_condition(...)` is a function call, so the nodes it selects cannot be counted",
                ),
                (
                    "orchestration/sensors/orders/late.py",
                    5,
                    31,
                    "sensor 'orders_late_sensor' names 4 nodes in its selection (limit 3)",
                ),
                (
                    "orchestration/sensors/orders/shared.py",
                    5,
                    31,
                    "sensor 'orders_shared_sensor' has a condition the rule cannot resolve: `READY` is imported from `shared_conditions`, which is not a project module, so the nodes it selects cannot be counted",
                ),
            ],
        },
        NamedSelectorLimitTestCase {
            description: "a CRLF condition module is read like any other, and a name follows its last binding before the use",
            files: vec![
                export_job(),
                (
                    "orchestration/sensors/orders/_crlf.py",
                    "# r\u{e9}sum\u{e9}\r\nfrom sqlbuild.orchestration import node_event\r\nX = m\u{e9}k(\"x\")\r\n".to_owned(),
                ),
                (
                    "orchestration/sensors/orders/_wide.py",
                    "from sqlbuild.orchestration import node_event\nWIDE = node_event(select=\"a b c d\", kinds=[])\n".to_owned(),
                ),
                (
                    "orchestration/sensors/orders/_rebound.py",
                    "from sqlbuild.orchestration import node_event\nWIDE = node_event(select=\"a\", kinds=[])\nfrom ._wide import WIDE\n".to_owned(),
                ),
                (
                    "orchestration/sensors/orders/crlf.py",
                    format!("from ._crlf import X\n{}", sensor("orders_crlf", "X")),
                ),
                (
                    "orchestration/sensors/orders/rebound.py",
                    format!("from ._rebound import WIDE\n{}", sensor("orders_rebound", "WIDE")),
                ),
                (
                    "orchestration/sensors/orders/reimported.py",
                    format!(
                        "from ._wide import WIDE\n{}from ._crlf import X as WIDE\n",
                        sensor("orders_reimported", "WIDE")
                    ),
                ),
            ],
            config: selected.clone(),
            expected_findings: vec![
                (
                    "orchestration/sensors/orders/crlf.py",
                    5,
                    31,
                    "sensor 'orders_crlf_sensor' has a condition the rule cannot resolve: in `orchestration/sensors/orders/_crlf.py`, `m\u{e9}k(...)` is a function call, so the nodes it selects cannot be counted",
                ),
                (
                    "orchestration/sensors/orders/rebound.py",
                    5,
                    31,
                    "sensor 'orders_rebound_sensor' names 4 nodes in its selection (limit 3)",
                ),
                (
                    "orchestration/sensors/orders/reimported.py",
                    5,
                    31,
                    "sensor 'orders_reimported_sensor' names 4 nodes in its selection (limit 3)",
                ),
            ],
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
            expected_findings: vec![],
        },
        NamedSelectorLimitTestCase {
            description: "without the rule selected, the declarations are not read",
            files: vec![(
                "orchestration/jobs/orders/nightly.toml",
                "not = [valid".to_owned(),
            )],
            config: json!({ "select": ["SQBRMODEL101"] }),
            expected_findings: vec![],
        },
    ];
    for test_case in test_cases {
        let findings = limit_findings(&test_case.files, test_case.config.clone());
        assert_eq!(
            findings,
            Ok(owned_findings(&test_case.expected_findings)),
            "{}",
            test_case.description
        );
    }
}
