use chrono::Utc;
use serde_json::json;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryKind;

use crate::native_commands::_helpers::location::changed_build::{
    changed_build_nodes, changed_build_settings, model_errors,
};
use crate::native_commands::_helpers::location::execution::metadata_sync;
use crate::native_commands::models::LocationMetadataSync;
use crate::native_commands::tests::helpers::{acknowledge_complete_pages, one_location};
use crate::native_commands::tests::test_types::{
    ChangedBuildNodesTestCase, ChangedBuildSettingsTestCase, ModelErrorsTestCase, ReplanTestCase,
};

#[test]
fn given_build_changed_when_frozen_like_the_server_then_only_the_changed_flag_is_set() {
    let test_cases = [ChangedBuildSettingsTestCase {
        description: "the default options, as the plan page's request freezes them",
        expected_settings: json!({"changed": true}),
    }];
    for test_case in test_cases {
        assert_eq!(
            serde_json::to_value(changed_build_settings().expect("frozen settings")).expect("json"),
            test_case.expected_settings,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_changed_plan_document_when_read_then_runs_split_into_changed_and_downstream() {
    let test_cases = [
        ChangedBuildNodesTestCase {
            description: "own changes and runs downstream of them",
            document: json!({"changed": {"counts": ["definition"], "reach": "job", "runs": [
                {"node": "model:orders", "reason": "definition changed", "downstream_of": null},
                {"node": "model:customers", "reason": "reads orders", "downstream_of": "model:orders"},
                {"node": "seed:inventory", "reason": "no record", "downstream_of": null},
            ], "held_back": [], "unchanged": ["model:products"]}}),
            expected_nodes: Ok((
                vec!["model:orders", "seed:inventory"],
                vec!["model:customers"],
            )),
        },
        ChangedBuildNodesTestCase {
            description: "nothing runs",
            document: json!({"changed": {"runs": [], "held_back": [], "unchanged": []}}),
            expected_nodes: Ok((vec![], vec![])),
        },
        ChangedBuildNodesTestCase {
            description: "a failed plan reports its message",
            document: json!({"error": {"code": "plan_failed", "message": "the synced warehouse copy is unavailable", "help": null}}),
            expected_nodes: Err("the synced warehouse copy is unavailable"),
        },
        ChangedBuildNodesTestCase {
            description: "a plan without --changed",
            document: json!({"models": []}),
            expected_nodes: Err("the plan has no --changed section"),
        },
    ];
    for test_case in test_cases {
        let actual = changed_build_nodes(&test_case.document);
        let actual = actual
            .as_ref()
            .map(|(changed, downstream)| {
                (
                    changed.iter().map(String::as_str).collect::<Vec<_>>(),
                    downstream.iter().map(String::as_str).collect::<Vec<_>>(),
                )
            })
            .map_err(String::as_str);
        assert_eq!(
            actual, test_case.expected_nodes,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_completed_syncs_when_deciding_to_replan_then_a_publication_an_applied_change_or_a_failed_plan_replans()
 {
    let test_cases = [
        ReplanTestCase {
            description: "the sync after a publication replans, then one that applied nothing does not",
            syncs: vec![(false, true), (false, true)],
            expected_replans: vec![true, false],
        },
        ReplanTestCase {
            description: "a sync that changed the copy replans",
            syncs: vec![(false, true), (true, true)],
            expected_replans: vec![true, true],
        },
        ReplanTestCase {
            description: "a plan that did not reach the server is retried after the next sync, then settles",
            syncs: vec![(false, true), (true, false), (false, true), (false, true)],
            expected_replans: vec![true, true, true, false],
        },
    ];
    for test_case in test_cases {
        let started = Utc::now();
        let mut state: LocationMetadataSync = metadata_sync((one_location(), 1), (3600, 0));
        state.applied = true;
        let (_, replans) = test_case.syncs.iter().fold(
            (state, Vec::new()),
            |(mut state, mut replans): (LocationMetadataSync, Vec<bool>), &(applied, reported)| {
                state.begin_sync(started);
                state.applied |= applied;
                let mut state = acknowledge_complete_pages(state, WarehouseHistoryKind::ALL.len());
                let replan: bool = state.take_replan();
                state.planned(reported || !replan);
                replans.push(replan);
                (state, replans)
            },
        );
        assert_eq!(
            replans, test_case.expected_replans,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_plan_with_models_it_could_not_plan_when_read_then_the_reason_names_each_model() {
    let test_cases = [
        ModelErrorsTestCase {
            description: "every model planned",
            document: json!({"models": [{"name": "orders", "action": "create_table"}]}),
            expected_reason: None,
        },
        ModelErrorsTestCase {
            description: "one model",
            document: json!({"models": [
                {"name": "orders", "action": "error", "error": {"code": "S203", "message": "model 'orders' sets full_refresh false", "help": null}},
                {"name": "totals", "action": "create_table"},
            ]}),
            expected_reason: Some(
                "Can't plan 1 model: orders: model 'orders' sets full_refresh false",
            ),
        },
        ModelErrorsTestCase {
            description: "several models",
            document: json!({"models": [
                {"name": "orders", "action": "error", "error": {"code": "S203", "message": "full_refresh false", "help": null}},
                {"name": "returns", "action": "error", "error": {"code": "S302", "message": "empty inputs", "help": null}},
            ]}),
            expected_reason: Some(
                "Can't plan 2 models: orders: full_refresh false; returns: empty inputs",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            model_errors(&test_case.document).as_deref(),
            test_case.expected_reason,
            "{}",
            test_case.description
        );
    }
}
