use chrono::Utc;
use serde_json::json;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryKind;

use crate::native_commands::_helpers::execution::metadata_sync;
use crate::native_commands::main::_location_changed_build::{
    changed_build_nodes, changed_build_settings,
};
use crate::native_commands::models::LocationMetadataSync;
use crate::native_commands::tests::helpers::{acknowledge_complete_pages, one_location};
use crate::native_commands::tests::test_types::ChangedBuildNodesTestCase;

#[test]
fn given_build_changed_when_frozen_like_the_server_then_only_the_changed_flag_is_set() {
    assert_eq!(
        serde_json::to_value(changed_build_settings().expect("frozen settings")).expect("json"),
        json!({"changed": true}),
        "the default options, as the plan page's request freezes them"
    );
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
            expected: Ok((
                vec!["model:orders", "seed:inventory"],
                vec!["model:customers"],
            )),
        },
        ChangedBuildNodesTestCase {
            description: "nothing runs",
            document: json!({"changed": {"runs": [], "held_back": [], "unchanged": []}}),
            expected: Ok((vec![], vec![])),
        },
        ChangedBuildNodesTestCase {
            description: "a failed plan reports its message",
            document: json!({"error": {"code": "plan_failed", "message": "the synced warehouse copy is unavailable", "help": null}}),
            expected: Err("the synced warehouse copy is unavailable"),
        },
        ChangedBuildNodesTestCase {
            description: "a plan without --changed",
            document: json!({"models": []}),
            expected: Err("the plan has no --changed section"),
        },
    ];
    for test_case in test_cases {
        let expected = test_case
            .expected
            .map(|(changed, downstream)| {
                (
                    changed.into_iter().map(str::to_owned).collect::<Vec<_>>(),
                    downstream
                        .into_iter()
                        .map(str::to_owned)
                        .collect::<Vec<_>>(),
                )
            })
            .map_err(str::to_owned);
        assert_eq!(
            changed_build_nodes(&test_case.document),
            expected,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_completed_syncs_when_deciding_to_replan_then_only_a_new_publication_or_an_applied_change_replans()
 {
    let started = Utc::now();
    let complete = |mut state: LocationMetadataSync, applied: bool| {
        state.begin_sync(started);
        state.applied |= applied;
        let mut state = acknowledge_complete_pages(state, WarehouseHistoryKind::ALL.len());
        assert!(state.idle, "the sync completed");
        let replan = state.take_replan();
        (state, replan)
    };
    let mut state = metadata_sync((one_location(), 1), (3600, 0));
    state.applied = true;
    let (state, after_publication) = complete(state, false);
    let (state, unchanged) = complete(state, false);
    let (_, changed) = complete(state, true);
    assert_eq!(
        [after_publication, unchanged, changed],
        [true, false, true],
        "the sync after a publication replans; a sync that applied nothing leaves the count; one that changed the copy replans"
    );
}
