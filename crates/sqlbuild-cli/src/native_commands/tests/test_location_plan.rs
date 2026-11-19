use serde_json::{Value, json};
use sqlbuild_orchestration::platform::constants::PLAN_DOCUMENT_BYTES;

use std::sync::Mutex;

use chrono::{TimeZone, Utc};

use crate::native_commands::_helpers::location::plan_document::plan_document;
use crate::native_commands::_helpers::location::plan_reuse::{remember_plan, reused_plan};
use crate::native_commands::_helpers::location::work::next_work_check;
use crate::native_commands::constants::LOCATION_WORK_IDLE_POLL;
use crate::native_commands::constants::REUSED_PLANS;
use crate::native_commands::models::LocationWorkTurn;
use crate::native_commands::models::PlanReuse;
use crate::native_commands::tests::helpers::{finished_plan, kept_plans, plan_output, reuse_key};
use crate::native_commands::tests::test_types::{
    PlanDocumentTestCase, PlanReuseBoundTestCase, PlanReuseTestCase, WorkCheckTestCase,
};

#[test]
fn given_plan_output_when_reading_it_on_a_location_then_diffs_always_travel_beside_the_document() {
    let test_cases = [
        PlanDocumentTestCase {
            description: "a small plan's diffs leave the document with their counts too",
            output: plan_output("+SELECT 2", 2),
            planned: true,
            expected_first_model: json!({"name": "model_0", "reason": "query_changed", "diff_stats": {
                "query": {"added": 1, "removed": 0, "whitespace_only": false},
                "config": {"added": 1, "removed": 0, "whitespace_only": false},
            }}),
            expected_diffs: (2, Some(vec!["+SELECT 2".to_owned()])),
        },
        PlanDocumentTestCase {
            description: "a plan over the document cap keeps every diff beside it",
            output: plan_output(
                &format!("+{}", "x".repeat(1024)),
                PLAN_DOCUMENT_BYTES / 1024 + 1,
            ),
            planned: true,
            expected_first_model: json!({"name": "model_0", "reason": "query_changed", "diff_stats": {
                "query": {"added": 1, "removed": 0, "whitespace_only": false},
                "config": {"added": 1, "removed": 0, "whitespace_only": false},
            }}),
            expected_diffs: (
                PLAN_DOCUMENT_BYTES / 1024 + 1,
                Some(vec![format!("+{}", "x".repeat(1024))]),
            ),
        },
        PlanDocumentTestCase {
            description: "a failed sqb plan is not read as a plan",
            output: plan_output("+SELECT 2", 1),
            planned: false,
            expected_first_model: Value::Null,
            expected_diffs: (0, None),
        },
    ];
    for test_case in test_cases {
        let (document, diffs) = plan_document(&test_case.output, test_case.planned);
        assert_eq!(
            (
                document["models"][0].clone(),
                diffs.len(),
                diffs
                    .get("model_0")
                    .and_then(|diff| diff.query_diff.clone()),
            ),
            (
                test_case.expected_first_model,
                test_case.expected_diffs.0,
                test_case.expected_diffs.1
            ),
            "{description}",
            description = test_case.description
        );
    }
}

#[test]
fn given_a_turn_of_location_work_when_scheduling_the_next_check_then_only_an_idle_turn_waits() {
    let test_cases = [
        WorkCheckTestCase {
            description: "a finished plan checks at once, as a person often plans again",
            claimed: true,
            sync: false,
            expected_wait: std::time::Duration::ZERO,
        },
        WorkCheckTestCase {
            description: "a requested sync checks at once too",
            claimed: true,
            sync: true,
            expected_wait: std::time::Duration::ZERO,
        },
        WorkCheckTestCase {
            description: "nothing to claim waits the idle interval",
            claimed: false,
            sync: false,
            expected_wait: LOCATION_WORK_IDLE_POLL,
        },
    ];
    let now = tokio::time::Instant::now();
    for test_case in test_cases {
        let turn = LocationWorkTurn {
            files: Vec::new(),
            sync: test_case.sync,
            claimed: test_case.claimed,
        };
        assert_eq!(
            next_work_check(&turn, now) - now,
            test_case.expected_wait,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_kept_plan_when_the_same_request_comes_again_then_only_the_same_generation_copy_and_request_reuse_it()
 {
    let reused_timings = json!({"planned_from": "synced_state", "reused_from": "2026-01-02T03:04:05Z",
        "total_seconds": 0.0,
        "steps": [{"name": "Reused plan from 2026-01-02T03:04:05Z", "seconds": 0.0}]});
    let test_cases = [
        PlanReuseTestCase {
            description: "the same generation, copy and request reuse the plan and say when it was made",
            kept: (3, "copy-1", false),
            asked: (3, "copy-1", &["orders"]),
            expected_timings: Some(reused_timings),
        },
        PlanReuseTestCase {
            description: "a newer synced copy plans again",
            kept: (3, "copy-1", false),
            asked: (3, "copy-2", &["orders"]),
            expected_timings: None,
        },
        PlanReuseTestCase {
            description: "a newer generation plans again",
            kept: (3, "copy-1", false),
            asked: (4, "copy-1", &["orders"]),
            expected_timings: None,
        },
        PlanReuseTestCase {
            description: "another selection plans again",
            kept: (3, "copy-1", false),
            asked: (3, "copy-1", &["customers"]),
            expected_timings: None,
        },
        PlanReuseTestCase {
            description: "a failed plan is not kept",
            kept: (3, "copy-1", true),
            asked: (3, "copy-1", &["orders"]),
            expected_timings: None,
        },
    ];
    let planned_at = Utc
        .with_ymd_and_hms(2026, 1, 2, 3, 4, 5)
        .single()
        .expect("time");
    for test_case in test_cases {
        let reuse: Mutex<Option<PlanReuse>> = Mutex::new(None);
        let (generation, version, failed) = test_case.kept;
        remember_plan(
            &reuse,
            (generation, version),
            reuse_key(&["orders"]),
            (&finished_plan(failed), planned_at),
        );
        let (generation, version, select) = test_case.asked;
        let reused = reused_plan(&reuse, (generation, version), &reuse_key(select));
        assert_eq!(
            reused.map(|(document, _)| document["timings"].clone()),
            test_case.expected_timings,
            "{description}",
            description = test_case.description
        );
    }
}

#[test]
fn given_more_finished_plans_than_kept_when_planning_then_the_oldest_is_dropped_and_a_newer_copy_drops_all()
 {
    let test_cases = [
        PlanReuseBoundTestCase {
            description: "the oldest of one plan too many is dropped",
            newer_copy: false,
            asked: ("model_0", "copy-1"),
            expected_reused: false,
        },
        PlanReuseBoundTestCase {
            description: "the next oldest is kept",
            newer_copy: false,
            asked: ("model_1", "copy-1"),
            expected_reused: true,
        },
        PlanReuseBoundTestCase {
            description: "a newer synced copy drops every kept plan",
            newer_copy: true,
            asked: ("model_1", "copy-2"),
            expected_reused: false,
        },
        PlanReuseBoundTestCase {
            description: "a plan of the newer copy is kept",
            newer_copy: true,
            asked: ("model_new", "copy-2"),
            expected_reused: true,
        },
    ];
    for test_case in test_cases {
        let reuse: Mutex<Option<PlanReuse>> = kept_plans(REUSED_PLANS + 1);
        let newer: &[(&str, &str)] =
            [&[][..], &[("model_new", "copy-2")][..]][usize::from(test_case.newer_copy)];
        for &(name, version) in newer {
            remember_plan(
                &reuse,
                (1, version),
                reuse_key(&[name]),
                (&finished_plan(false), Utc::now()),
            );
        }
        let (name, version) = test_case.asked;
        assert_eq!(
            reused_plan(&reuse, (1, version), &reuse_key(&[name])).is_some(),
            test_case.expected_reused,
            "{description}",
            description = test_case.description
        );
    }
}
