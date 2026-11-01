use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::declarations::main::change_trigger::change_trigger;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::platform::models::PlatformEvent;
use crate::sensors::models::SensorEvaluationFact;
use crate::snapshots::models::SnapshotInput;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::models::{ControlEvent, ControlStore, TransactionMode};

use crate::runs::tests::helpers::snapshot as base_snapshot;
use crate::store::tests::helpers::read_recall_events;

pub(crate) fn snapshot(location_id: Uuid) -> SnapshotInput {
    let mut input = base_snapshot(location_id);
    input.documents.insert(
        "jobs".into(),
        json!({
            "orders": {"description": "Refresh orders", "selection": ["orders"], "overlap": "allow"}
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
    read_recall_events(store, 0, 1000)
        .await
        .expect("read durable history")
        .into_iter()
        .filter(|row| row["event_type"] == "sensor_evaluation")
        .map(|row| serde_json::from_value(row["payload"].clone()).expect("typed attempt fact"))
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
