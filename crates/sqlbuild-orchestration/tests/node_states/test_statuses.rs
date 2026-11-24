//! Every node gets one build status on the server, from its build, its last run when it has no table, and its upstream.

use std::collections::HashMap;

use serde_json::json;

use crate::node_states::_helpers::reads::{loaded_sources, newest_attempt};
use crate::node_states::models::NodeBuildStatus;
use crate::node_states::tests::helpers::{at, outcome, project_statuses};
use crate::node_states::tests::test_types::{LoaderAttemptTestCase, NodeStatusTestCase};
use crate::snapshots::models::SnapshotNode;
use crate::warehouse_metadata::models::CatalogNodeOutcome;

#[test]
fn given_node_states_when_statuses_are_worked_out_then_each_node_has_one() {
    let test_cases = [
        NodeStatusTestCase {
            description: "a source is a source, whatever ran",
            node_key: "source:raw",
            expected_status: NodeBuildStatus::Source,
            expected_status_at: None,
        },
        NodeStatusTestCase {
            description: "a table built after its upstream is current",
            node_key: "model:fresh",
            expected_status: NodeBuildStatus::Current,
            expected_status_at: Some(5),
        },
        NodeStatusTestCase {
            description: "a table built before its upstream's newer build is behind",
            node_key: "model:old",
            expected_status: NodeBuildStatus::Behind,
            expected_status_at: Some(2),
        },
        NodeStatusTestCase {
            description: "a view reads its upstream when queried, so it is never behind",
            node_key: "model:view",
            expected_status: NodeBuildStatus::Current,
            expected_status_at: Some(1),
        },
        NodeStatusTestCase {
            description: "a table over a view is behind the data behind the view",
            node_key: "model:over_view",
            expected_status: NodeBuildStatus::Behind,
            expected_status_at: Some(3),
        },
        NodeStatusTestCase {
            description: "a definition changed since the build is changed",
            node_key: "model:edited",
            expected_status: NodeBuildStatus::Changed,
            expected_status_at: Some(4),
        },
        NodeStatusTestCase {
            description: "a failed attempt outranks a changed definition",
            node_key: "model:edited_failed",
            expected_status: NodeBuildStatus::Failed,
            expected_status_at: Some(6),
        },
        NodeStatusTestCase {
            description: "a table never built is not built",
            node_key: "model:unbuilt",
            expected_status: NodeBuildStatus::NotBuilt,
            expected_status_at: None,
        },
        NodeStatusTestCase {
            description: "a table whose only attempt failed is failed",
            node_key: "model:unbuilt_failed",
            expected_status: NodeBuildStatus::Failed,
            expected_status_at: Some(6),
        },
        NodeStatusTestCase {
            description: "a node without a table that ran successfully is current, as of its run",
            node_key: "asset:ran",
            expected_status: NodeBuildStatus::Current,
            expected_status_at: Some(7),
        },
        NodeStatusTestCase {
            description: "a run with warnings succeeded",
            node_key: "asset:warned",
            expected_status: NodeBuildStatus::Current,
            expected_status_at: Some(8),
        },
        NodeStatusTestCase {
            description: "a node without a table whose run failed is failed",
            node_key: "asset:failed",
            expected_status: NodeBuildStatus::Failed,
            expected_status_at: Some(9),
        },
        NodeStatusTestCase {
            description: "a node without a table that never ran is not built",
            node_key: "asset:never",
            expected_status: NodeBuildStatus::NotBuilt,
            expected_status_at: None,
        },
        NodeStatusTestCase {
            description: "views reading each other end the walk",
            node_key: "model:loop_a",
            expected_status: NodeBuildStatus::Current,
            expected_status_at: Some(1),
        },
    ];
    let states = project_statuses();
    for test_case in test_cases {
        let state = &states[test_case.node_key];
        assert_eq!(
            (state.status, state.status_at),
            (
                test_case.expected_status,
                test_case.expected_status_at.map(at)
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_loader_when_its_sources_ran_then_its_attempt_is_theirs() {
    let test_cases = [
        LoaderAttemptTestCase {
            description: "a loader's run is recorded under the source it loads",
            attempts: &[("source:raw__orders", "success", 2)],
            expected_attempt: Some(("success", 2)),
        },
        LoaderAttemptTestCase {
            description: "the newest of its sources' attempts",
            attempts: &[
                ("source:raw__orders", "failed", 1),
                ("source:raw__customers", "success", 3),
            ],
            expected_attempt: Some(("success", 3)),
        },
        LoaderAttemptTestCase {
            description: "a failure first when its sources ran at once",
            attempts: &[
                ("source:raw__orders", "success", 4),
                ("source:raw__customers", "failed", 4),
            ],
            expected_attempt: Some(("failed", 4)),
        },
        LoaderAttemptTestCase {
            description: "another loader's source is not its own",
            attempts: &[("source:other", "success", 5)],
            expected_attempt: None,
        },
    ];
    let source = |loader: &str| SnapshotNode {
        kind: "source".to_owned(),
        definition: json!({"name": "source", "entry": {"loader": loader}}),
        raw_sql: None,
        compiled_sql: None,
        python_source: None,
    };
    let loader = SnapshotNode {
        kind: "loader".to_owned(),
        definition: json!({"name": "raw"}),
        raw_sql: None,
        compiled_sql: None,
        python_source: None,
    };
    let nodes = vec![
        ("source:raw__orders".to_owned(), source("raw")),
        ("source:raw__customers".to_owned(), source("raw")),
        ("source:other".to_owned(), source("other")),
    ];
    let loaded = loaded_sources(&nodes);
    for test_case in test_cases {
        let attempts: HashMap<String, CatalogNodeOutcome> = test_case
            .attempts
            .iter()
            .map(|(key, status, hours)| {
                let attempt = outcome(status, *hours).expect("an outcome for every attempt");
                ((*key).to_owned(), attempt)
            })
            .collect();
        let found = newest_attempt(&attempts, "loader:raw", &loader, &loaded);
        assert_eq!(
            found.map(|attempt| (attempt.status, attempt.at)),
            test_case
                .expected_attempt
                .map(|(status, hours)| (status.to_owned(), at(hours))),
            "{}",
            test_case.description
        );
    }
}
