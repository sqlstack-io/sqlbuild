use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::declarations::main::change_trigger::change_trigger;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::platform::models::PlatformEvent;
use crate::runs::main::{
    advance_groups::advance_groups, change_run::change_run, claim_next_run::claim_next_run,
    request_job_run::request_job_run,
};
use crate::runs::models::{
    JobRunRequest, NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunOutcome,
};
use crate::scheduler::models::SchedulerLease;
use crate::sensors::models::SensorEvaluationFact;
use crate::snapshots::models::SnapshotInput;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlStore, TransactionMode};

use crate::runs::tests::helpers::snapshot as base_snapshot;

pub(crate) fn snapshot(location_id: Uuid) -> SnapshotInput {
    let mut input = base_snapshot(location_id);
    input.documents.insert(
        "jobs".into(),
        json!({
            "orders": {"description": "Refresh orders", "selection": ["orders"]}
        }),
    );
    input.documents.insert(
        "triggers".into(),
        json!({
            "health_orders": {
                "description": "Refresh reporting on overdue ticks", "job_key": "orders",
                "when": {"kind": "platform_event", "kinds": ["tick_overdue", "location_broken", "launch_failed"]},
                "minimum_interval_seconds": 1
            }
        }),
    );
    input
}

pub(crate) async fn append_events(
    store: &ControlStore,
    indices: std::ops::Range<u128>,
    occurred_at: DateTime<Utc>,
) {
    let mut transaction = begin(store, TransactionMode::Write)
        .await
        .expect("event transaction");
    let payloads = [
        PlatformEvent::TickOverdue {
            trigger_id: Uuid::from_u128(7),
            scheduled_for: occurred_at,
        },
        PlatformEvent::Heartbeat {
            replica_id: Uuid::from_u128(8),
        },
        PlatformEvent::LocationBroken {
            location_id: Uuid::from_u128(9),
            target: "development".into(),
            last_seen_at: occurred_at,
        },
        PlatformEvent::Heartbeat {
            replica_id: Uuid::from_u128(10),
        },
        PlatformEvent::LaunchFailed {
            run_id: Uuid::from_u128(11),
            claim_id: Uuid::from_u128(12),
            code: "local_spawn_failed".into(),
        },
        PlatformEvent::Heartbeat {
            replica_id: Uuid::from_u128(13),
        },
    ];
    for index in indices {
        let platform = &payloads[(index % 6) as usize];
        let (next, appended) = append_control_event(
            transaction,
            ControlEvent {
                event_id: Uuid::from_u128(1000 + index),
                invocation_id: "platform/test-health".into(),
                event_type: platform.event_type(),
                producer: "platform",
                occurred_at,
                run_id: None,
                payload: json!({"platform": platform}),
            },
        )
        .await
        .expect("append platform observation");
        assert!(appended.inserted);
        transaction = next;
    }
    commit(transaction).await.expect("commit observations");
}

pub(crate) async fn evaluations(store: &ControlStore) -> Vec<SensorEvaluationFact> {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        "SELECT fact_json FROM sqb_sensor_evaluation_events ORDER BY occurred_at, event_id",
        vec![],
    )
    .await
    .expect("evaluation history");
    commit(transaction).await.expect("commit");
    rows.into_iter()
        .map(|row| serde_json::from_value(row["fact_json"].clone()).expect("typed attempt fact"))
        .collect()
}

pub(crate) async fn change(store: &ControlStore, trigger_id: Uuid, change: TriggerChange) {
    let event_id = Uuid::now_v7();
    let context = AuditContext {
        actor: "test-admin".into(),
        granting_role: "admin".into(),
        request_id: event_id,
        ip_address: None,
    };
    let request = TriggerChangeRequest {
        event_id,
        trigger_id,
        actor: context.actor.clone(),
        reason: "Test operator change".into(),
        change,
    };
    assert!(
        change_trigger(store, &context, &request)
            .await
            .expect("apply operator fact")
    );
    assert!(
        !change_trigger(store, &context, &request)
            .await
            .expect("retry operator fact")
    );
}

/// Real request, claim, node facts and finish, including the virtual group parent.
pub(crate) async fn finish_run(
    store: &ControlStore,
    lease: &SchedulerLease,
    scope: (Uuid, &str),
    job_key: &str,
    outcome: RunOutcome,
    nodes: [NodeOutcome; 2],
    started: [bool; 2],
) -> Uuid {
    let run_id = Uuid::now_v7();
    let context = AuditContext {
        actor: "test-admin".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    request_job_run(
        store,
        &context,
        &JobRunRequest {
            run_id,
            location_id: scope.0,
            target: scope.1.into(),
            job_key: job_key.into(),
            reason: "Exercise terminal sensor scope".into(),
            priority: 10,
            window: None,
        },
    )
    .await
    .expect("request source run");
    finish_claimed_run(store, lease, scope, run_id, outcome, nodes, started).await;
    run_id
}

/// Execute only the frozen selection, including a from-failure rerun's reduced scope.
pub(crate) async fn finish_claimed_run(
    store: &ControlStore,
    lease: &SchedulerLease,
    scope: (Uuid, &str),
    run_id: Uuid,
    outcome: RunOutcome,
    nodes: [NodeOutcome; 2],
    started: [bool; 2],
) {
    let limits = std::collections::BTreeMap::new();
    advance_groups(store, lease, (&limits, scope))
        .await
        .expect("advance parent");
    let claim = claim_next_run(store, lease, &limits, Some(scope))
        .await
        .expect("claim source")
        .expect("source ready");
    assert!(claim.request.run_id == run_id || claim.request.parent_run_id == Some(run_id));
    let worker = RunAuthority::Worker {
        run_id: claim.request.run_id,
        claim_id: claim.claim_id,
    };
    let mut changes = vec![RunChange::Started];
    for (index, key) in ["customers", "orders"]
        .into_iter()
        .enumerate()
        .filter(|(_, key)| claim.request.selection.contains(*key))
    {
        changes.extend(started[index].then_some(RunChange::NodeStarted {
            node_key: key.into(),
        }));
        changes.push(RunChange::NodeFinished {
            node_key: key.into(),
            outcome: nodes[index],
            rows_affected: None,
            error_code: None,
            skipped_because_of: None,
        });
    }
    for change in changes {
        change_run(
            store,
            worker,
            &RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: claim.request.run_id,
                claim_id: Some(claim.claim_id),
                change,
            },
        )
        .await
        .expect("publish worker evidence");
    }
    let terminal = RunChangeRequest {
        event_id: Uuid::now_v7(),
        run_id: claim.request.run_id,
        claim_id: Some(claim.claim_id),
        change: RunChange::Finished {
            outcome,
            reason: "Terminal outcome fixture".into(),
            exit_code: None,
        },
    };
    assert!(
        change_run(store, RunAuthority::Scheduler(lease), &terminal)
            .await
            .expect("finish")
    );
    assert!(
        !change_run(store, RunAuthority::Scheduler(lease), &terminal)
            .await
            .expect("retry finish")
    );
    advance_groups(store, lease, (&limits, scope))
        .await
        .expect("complete parent");
}
