//! The server decides a sync's state and a target's verdict, so every page reads one rule: what stops runs is red.

use chrono::TimeDelta;
use uuid::Uuid;

use crate::platform::_helpers::health::failing_syncs::{stalled_sync, sync_state};
use crate::platform::models::{
    LocationFact, LocationState, LocationVerdict, ProblemTier, WarehouseReadFailure,
    WarehouseSyncFailure,
};
use crate::platform::tests::helpers::{at, sync, sync_now, sync_report};
use crate::platform::tests::test_types::{LocationVerdictTestCase, SyncStateTestCase};

#[test]
fn given_sync_reports_when_read_then_a_running_sync_names_its_start_and_stalls_after_ten_minutes() {
    let started = at(0) - TimeDelta::milliseconds(10);
    let unreached = Some(WarehouseReadFailure::Unreachable {});
    let test_cases = [
        SyncStateTestCase {
            description: "a sync that moved forward five minutes ago is running",
            observations: vec![sync(0, None, (None, false))],
            now_minutes: 5,
            expected_running: Some((started, false)),
            expected_failing: false,
            expected_stalled_problem: false,
        },
        SyncStateTestCase {
            description: "ten minutes without progress is a stall",
            observations: vec![sync(0, None, (None, false))],
            now_minutes: 10,
            expected_running: Some((started, true)),
            expected_failing: false,
            expected_stalled_problem: true,
        },
        SyncStateTestCase {
            description: "a completed sync runs no longer, however old its last page",
            observations: vec![sync_report(0, None, (None, false, true))],
            now_minutes: 60,
            expected_running: None,
            expected_failing: false,
            expected_stalled_problem: false,
        },
        SyncStateTestCase {
            description: "a failing sync is failing, not running or stalled",
            observations: vec![sync(
                0,
                Some(WarehouseSyncFailure::Read),
                (unreached, false),
            )],
            now_minutes: 60,
            expected_running: None,
            expected_failing: true,
            expected_stalled_problem: false,
        },
        SyncStateTestCase {
            description: "a stopped sync is failing",
            observations: vec![sync(0, None, (None, true))],
            now_minutes: 60,
            expected_running: None,
            expected_failing: true,
            expected_stalled_problem: false,
        },
        SyncStateTestCase {
            description: "a sync waiting on another isn't failing",
            observations: vec![sync(0, Some(WarehouseSyncFailure::Busy), (None, false))],
            now_minutes: 60,
            expected_running: None,
            expected_failing: false,
            expected_stalled_problem: false,
        },
        SyncStateTestCase {
            description: "a target that never synced has nothing to show",
            observations: vec![],
            now_minutes: 60,
            expected_running: None,
            expected_failing: false,
            expected_stalled_problem: false,
        },
    ];
    for test_case in test_cases {
        let now = at(test_case.now_minutes);
        let state = sync_state(&test_case.observations, ("prod", now));
        assert_eq!(
            (
                state
                    .running
                    .as_ref()
                    .map(|running| (running.started_at, running.stalled)),
                state.failing,
                state.requested,
            ),
            (test_case.expected_running, test_case.expected_failing, None),
            "{}",
            test_case.description
        );
        assert_eq!(
            stalled_sync(
                &test_case.observations,
                (Uuid::nil(), "orders", "prod"),
                now
            )
            .map(|stalled| (stalled.progressed_at, stalled.tier)),
            test_case
                .expected_stalled_problem
                .then_some((at(0), ProblemTier::NeedsAttention)),
            "{}: the stall is a problem that needs attention",
            test_case.description
        );
    }
}

#[test]
fn given_a_target_when_judged_then_its_worst_open_fact_decides_and_only_what_stops_runs_is_red() {
    let test_cases = [
        LocationVerdictTestCase {
            description: "a failed connection check leads, its sync failure only follows from it",
            state: LocationState::Degraded,
            published: true,
            sync: sync_now(true, None),
            tested_minutes: Some(1),
            expected_fact: LocationFact::CantConnect,
            expected_tier: Some(ProblemTier::StopsRuns),
        },
        LocationVerdictTestCase {
            description: "a location that doesn't check in stops runs",
            state: LocationState::Broken,
            published: true,
            sync: sync_now(false, Some(true)),
            tested_minutes: Some(1),
            expected_fact: LocationFact::NotCheckingIn,
            expected_tier: Some(ProblemTier::StopsRuns),
        },
        LocationVerdictTestCase {
            description: "a failed deploy with no version to fall back on stops runs",
            state: LocationState::FallingBack,
            published: false,
            sync: sync_now(false, None),
            tested_minutes: None,
            expected_fact: LocationFact::DeployFailed,
            expected_tier: Some(ProblemTier::StopsRuns),
        },
        LocationVerdictTestCase {
            description: "a failed deploy still running the previous version needs attention",
            state: LocationState::FallingBack,
            published: true,
            sync: sync_now(true, None),
            tested_minutes: Some(1),
            expected_fact: LocationFact::DeployFailedOnPrevious,
            expected_tier: Some(ProblemTier::NeedsAttention),
        },
        LocationVerdictTestCase {
            description: "a failing sync outranks a failed change plan",
            state: LocationState::ChangePlanFailed,
            published: true,
            sync: sync_now(true, None),
            tested_minutes: Some(1),
            expected_fact: LocationFact::SyncFailing,
            expected_tier: Some(ProblemTier::NeedsAttention),
        },
        LocationVerdictTestCase {
            description: "a failed change plan needs attention",
            state: LocationState::ChangePlanFailed,
            published: true,
            sync: sync_now(false, Some(false)),
            tested_minutes: Some(1),
            expected_fact: LocationFact::ChangePlanFailed,
            expected_tier: Some(ProblemTier::NeedsAttention),
        },
        LocationVerdictTestCase {
            description: "a failed discover function needs attention",
            state: LocationState::DiscoveryFailed,
            published: true,
            sync: sync_now(false, None),
            tested_minutes: Some(1),
            expected_fact: LocationFact::DiscoveryFailed,
            expected_tier: Some(ProblemTier::NeedsAttention),
        },
        LocationVerdictTestCase {
            description: "a stalled sync on a healthy location needs attention",
            state: LocationState::Healthy,
            published: true,
            sync: sync_now(false, Some(true)),
            tested_minutes: Some(1),
            expected_fact: LocationFact::SyncStalled,
            expected_tier: Some(ProblemTier::NeedsAttention),
        },
        LocationVerdictTestCase {
            description: "a connection last tested before the published version is untested",
            state: LocationState::Healthy,
            published: true,
            sync: sync_now(false, None),
            tested_minutes: Some(-1),
            expected_fact: LocationFact::NotTested,
            expected_tier: Some(ProblemTier::NeedsAttention),
        },
        LocationVerdictTestCase {
            description: "a connection never tested is untested",
            state: LocationState::Healthy,
            published: true,
            sync: sync_now(false, None),
            tested_minutes: None,
            expected_fact: LocationFact::NotTested,
            expected_tier: Some(ProblemTier::NeedsAttention),
        },
        LocationVerdictTestCase {
            description: "a starting location is only starting",
            state: LocationState::Starting,
            published: true,
            sync: sync_now(false, None),
            tested_minutes: None,
            expected_fact: LocationFact::Starting,
            expected_tier: Some(ProblemTier::NeedsAttention),
        },
        LocationVerdictTestCase {
            description: "healthy only when nothing is open",
            state: LocationState::Healthy,
            published: true,
            sync: sync_now(false, Some(false)),
            tested_minutes: Some(1),
            expected_fact: LocationFact::Healthy,
            expected_tier: None,
        },
    ];
    for test_case in test_cases {
        let verdict = LocationVerdict::of(
            (test_case.state, test_case.published.then(|| at(0))),
            &test_case.sync,
            test_case.tested_minutes.map(at),
        );
        assert_eq!(
            verdict,
            LocationVerdict {
                fact: test_case.expected_fact,
                tier: test_case.expected_tier,
            },
            "{}",
            test_case.description
        );
    }
}
