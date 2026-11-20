use std::collections::BTreeMap;

use serde_json::{Value, json};
use sqlbuild_orchestration::platform::constants::PLAN_DOCUMENT_BYTES;
use sqlbuild_orchestration::runs::models::PinnedJob;

use std::sync::Mutex;

use chrono::TimeDelta;

use crate::native_commands::_helpers::location::plan_document::plan_document;
use crate::native_commands::_helpers::location::plan_request::plan_arguments;
use crate::native_commands::_helpers::location::plan_reuse::{remember_plan, reused_plan};
use crate::native_commands::_helpers::location::work::next_work_check;
use crate::native_commands::constants::LOCATION_WORK_IDLE_POLL;
use crate::native_commands::constants::{REUSED_PLAN_BYTES, REUSED_PLANS};
use crate::native_commands::models::LocationServe;
use crate::native_commands::models::LocationWorkTurn;
use crate::native_commands::models::PlanReuse;
use crate::native_commands::models::{PlanKey, PlanToKeep};
use crate::native_commands::tests::helpers::{
    finished_plan, finished_plan_bytes, kept_plans, plan_output, plan_to_keep, reuse_clock,
    reuse_key,
};
use crate::native_commands::tests::test_types::PinnedReuseTestCase;
use crate::native_commands::tests::test_types::{
    PlanArgumentsTestCase, PlanDocumentTestCase, PlanReuseBoundTestCase, PlanReuseTestCase,
    WorkCheckTestCase,
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
fn given_a_kept_plan_when_the_same_request_comes_again_then_only_the_same_generation_copy_and_request_reuse_it_for_ten_minutes()
 {
    let reused_timings = json!({"planned_from": "synced_state", "reused_from": "2026-01-02T03:04:05Z",
        "total_seconds": 4.2, "steps": [{"name": "Detecting changes", "seconds": 4.2}]});
    let test_cases = [
        PlanReuseTestCase {
            description: "the same generation, copy and request reuse the plan and say when it was made",
            kept: (3, "copy-1", false),
            asked: (3, "copy-1", &["orders"]),
            age: TimeDelta::seconds(599),
            expected_timings: Some(reused_timings),
        },
        PlanReuseTestCase {
            description: "a plan ten minutes old is not reused, so windows and expiry follow the clock",
            kept: (3, "copy-1", false),
            asked: (3, "copy-1", &["orders"]),
            age: TimeDelta::minutes(10),
            expected_timings: None,
        },
        PlanReuseTestCase {
            description: "a newer synced copy plans again",
            kept: (3, "copy-1", false),
            asked: (3, "copy-2", &["orders"]),
            age: TimeDelta::zero(),
            expected_timings: None,
        },
        PlanReuseTestCase {
            description: "a newer generation plans again",
            kept: (3, "copy-1", false),
            asked: (4, "copy-1", &["orders"]),
            age: TimeDelta::zero(),
            expected_timings: None,
        },
        PlanReuseTestCase {
            description: "another selection plans again",
            kept: (3, "copy-1", false),
            asked: (3, "copy-1", &["customers"]),
            age: TimeDelta::zero(),
            expected_timings: None,
        },
        PlanReuseTestCase {
            description: "a failed plan is not kept",
            kept: (3, "copy-1", true),
            asked: (3, "copy-1", &["orders"]),
            age: TimeDelta::zero(),
            expected_timings: None,
        },
    ];
    for test_case in test_cases {
        let reuse: Mutex<Option<PlanReuse>> = Mutex::new(None);
        let (generation, version, failed) = test_case.kept;
        let (document, diffs) = finished_plan(failed);
        remember_plan(
            (&reuse, (REUSED_PLANS, REUSED_PLAN_BYTES)),
            plan_to_keep(generation, version, &["orders"]),
            (&document, &diffs),
        );
        let (generation, version, select) = test_case.asked;
        let reused = reused_plan(
            &reuse,
            (generation, version),
            &reuse_key(select),
            reuse_clock() + test_case.age,
        );
        assert_eq!(
            reused.map(|(document, _)| document["timings"].clone()),
            test_case.expected_timings,
            "{description}",
            description = test_case.description
        );
    }
}

#[test]
fn given_a_kept_plan_when_a_request_pins_other_jobs_then_it_plans_again() {
    let pinned = |settings: serde_json::Value| -> BTreeMap<String, PinnedJob> {
        BTreeMap::from([(
            "nightly".to_owned(),
            PinnedJob {
                selection: ["model:orders".to_owned()].into(),
                settings: serde_json::from_value(settings).expect("settings"),
            },
        )])
    };
    let test_cases = [
        PinnedReuseTestCase {
            description: "the same pinned jobs reuse the plan",
            kept: pinned(json!({"full_refresh": true})),
            asked: pinned(json!({"full_refresh": true})),
            expected_reused: true,
        },
        PinnedReuseTestCase {
            description: "a pinned job with other settings plans again",
            kept: pinned(json!({"full_refresh": true})),
            asked: pinned(json!({"full_refresh": false})),
            expected_reused: false,
        },
        PinnedReuseTestCase {
            description: "a request that pins no job does not reuse one that pinned a job",
            kept: pinned(json!({"full_refresh": true})),
            asked: BTreeMap::new(),
            expected_reused: false,
        },
    ];
    for test_case in test_cases {
        let reuse: Mutex<Option<PlanReuse>> = Mutex::new(None);
        let (document, diffs) = finished_plan(false);
        let mut keep: PlanToKeep = plan_to_keep(3, "copy-1", &["orders"]);
        keep.key.jobs = test_case.kept;
        remember_plan(
            (&reuse, (REUSED_PLANS, REUSED_PLAN_BYTES)),
            keep,
            (&document, &diffs),
        );
        let mut asked: PlanKey = reuse_key(&["orders"]);
        asked.jobs = test_case.asked;
        let reused = reused_plan(&reuse, (3, "copy-1"), &asked, reuse_clock());
        assert_eq!(
            reused.is_some(),
            test_case.expected_reused,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_more_finished_plans_than_kept_when_planning_then_the_oldest_is_dropped_and_a_newer_copy_drops_all()
 {
    let plan_bytes: usize = finished_plan_bytes();
    let test_cases = [
        PlanReuseBoundTestCase {
            description: "the oldest of one plan too many is dropped",
            limits: (REUSED_PLANS, REUSED_PLAN_BYTES, REUSED_PLANS + 1),
            newer_copy: false,
            asked: ("model_0", "copy-1"),
            expected_reused: false,
        },
        PlanReuseBoundTestCase {
            description: "the next oldest is kept",
            limits: (REUSED_PLANS, REUSED_PLAN_BYTES, REUSED_PLANS + 1),
            newer_copy: false,
            asked: ("model_1", "copy-1"),
            expected_reused: true,
        },
        PlanReuseBoundTestCase {
            description: "past the byte limit the oldest plan is dropped",
            limits: (REUSED_PLANS, 2 * plan_bytes, 3),
            newer_copy: false,
            asked: ("model_0", "copy-1"),
            expected_reused: false,
        },
        PlanReuseBoundTestCase {
            description: "the plans within the byte limit are kept",
            limits: (REUSED_PLANS, 2 * plan_bytes, 3),
            newer_copy: false,
            asked: ("model_1", "copy-1"),
            expected_reused: true,
        },
        PlanReuseBoundTestCase {
            description: "a plan larger than the byte limit is not kept",
            limits: (REUSED_PLANS, plan_bytes - 1, 1),
            newer_copy: false,
            asked: ("model_0", "copy-1"),
            expected_reused: false,
        },
        PlanReuseBoundTestCase {
            description: "a newer synced copy drops every kept plan",
            limits: (REUSED_PLANS, REUSED_PLAN_BYTES, REUSED_PLANS + 1),
            newer_copy: true,
            asked: ("model_1", "copy-2"),
            expected_reused: false,
        },
        PlanReuseBoundTestCase {
            description: "a plan of the newer copy is kept",
            limits: (REUSED_PLANS, REUSED_PLAN_BYTES, REUSED_PLANS + 1),
            newer_copy: true,
            asked: ("model_new", "copy-2"),
            expected_reused: true,
        },
    ];
    let (document, diffs) = finished_plan(false);
    for test_case in test_cases {
        let (most_plans, most_bytes, count) = test_case.limits;
        let reuse: Mutex<Option<PlanReuse>> = kept_plans(count, (most_plans, most_bytes));
        let newer: &[(&str, &str)] =
            [&[][..], &[("model_new", "copy-2")][..]][usize::from(test_case.newer_copy)];
        for &(name, version) in newer {
            remember_plan(
                (&reuse, (most_plans, most_bytes)),
                plan_to_keep(1, version, &[name]),
                (&document, &diffs),
            );
        }
        let (name, version) = test_case.asked;
        assert_eq!(
            reused_plan(&reuse, (1, version), &reuse_key(&[name]), reuse_clock()).is_some(),
            test_case.expected_reused,
            "{description}",
            description = test_case.description
        );
    }
}

#[test]
fn given_frozen_plan_settings_when_building_the_plan_arguments_then_run_request_settings_never_vary_a_kept_plan()
 {
    let test_cases = [
        PlanArgumentsTestCase {
            description: "build settings become the plan arguments the kept plan is keyed on",
            settings: json!({"changed": true}),
            expected_refused: false,
        },
        PlanArgumentsTestCase {
            description: "--no-job-settings belongs to the run request and never reaches a plan",
            settings: json!({"changed": true, "job_settings": false}),
            expected_refused: true,
        },
    ];
    let options = LocationServe {
        project_dir: "/project".into(),
        server_url: "http://127.0.0.1:1".into(),
        location_id: uuid::Uuid::nil(),
        target: "development".into(),
        image_id: None,
        metadata_reconcile_seconds: 3600,
        safety_sync_seconds: 0,
        dev_events: false,
    };
    for test_case in test_cases {
        let settings = serde_json::from_value(test_case.settings).expect("settings");
        let argv: Result<Vec<String>, String> = plan_arguments(&options, &settings);
        assert_eq!(
            argv.is_err(),
            test_case.expected_refused,
            "{description}: {argv:?}",
            description = test_case.description
        );
    }
}
