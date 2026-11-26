use crate::scheduler::tests::contracts::ticks::helpers::calendar_text;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;

use crate::platform::models::{
    LocationHealth, LocationHealthObservation, LocationHealthReport, PlatformEvent,
    PlatformEventEnvelope,
};
use crate::runs::main::execution::claim_next_run::claim_next_run;
use crate::runs::main::execution::publish_run_recall::publish_run_recall;
use crate::runs::models::{RecallBatch, RecallRecord};
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;
use crate::store::tests::helpers::read_recall_events;
use chrono::{TimeDelta, Utc};
use serde_json::json;
use sqlbuild_recall::records::models::RecallValue;
use sqlbuild_recall::schema::models::Table;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::platform::tests::test_types::{
    Instance, Observation, Reported, ReportedHealth, WebhookReceiver,
};
use crate::runs::tests::helpers::{queue, request};
use crate::scheduler::tests::contracts::helpers::acquire_lease;

pub(crate) fn snapshot(location_id: Uuid) -> SnapshotInput {
    let future = Utc::now() + TimeDelta::days(10);
    let mut triggers = BTreeMap::new();
    for index in 0..101 {
        triggers.insert(
            format!("daily_orders_{index}"),
            json!({
                "description": "Refresh orders", "job_key": "orders",
                "starts_at": calendar_text(0),
                "schedule": {"kind": "calendar", "times": [calendar_text(0), future],
                    "refresh_at": calendar_text(0)}
            }),
        );
    }
    for (name, status) in [("expired_orders", "running"), ("paused_orders", "paused")] {
        triggers.insert(name.into(), json!({
            "description": "Refresh orders", "job_key": "orders", "default_status": status,
            "starts_at": calendar_text(0),
            "schedule": {"kind": "calendar", "times": [calendar_text(0)], "refresh_at": calendar_text(0)}
        }));
    }
    triggers.insert(
        "future_orders".into(),
        json!({
            "description": "Future orders refresh", "job_key": "orders",
            "schedule": {"kind": "calendar", "times": [future], "refresh_at": future}
        }),
    );
    SnapshotInput {
        location_id,
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: false,
        git_author: None,
        image_id: None,
        environment_hash: "orders-watchdog-environment".into(),
        sqb_version: "1.0.0".into(),
        nodes: BTreeMap::from([(
            "orders".into(),
            SnapshotNode {
                kind: "model".into(),
                definition: json!({"materialization": "table"}),
                raw_sql: Some("select 1 as order_id".into()),
                compiled_sql: Some("select 1 as order_id".into()),
                python_source: None,
            },
        )]),
        documents: BTreeMap::from([
            (
                "jobs".into(),
                json!({"orders": {"description": "Build orders", "selection": ["orders"]}}),
            ),
            ("triggers".into(), json!(triggers)),
        ]),
    }
}

pub(crate) async fn wait_for_scans(
    store: &ControlStore,
    replicas: &BTreeSet<Uuid>,
    tasks: &[JoinHandle<Result<(), ControlError>>],
) -> Vec<Observation> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut cursor = 0;
        let mut observations = Vec::new();
        let mut scanned = BTreeSet::new();
        for _ in 0..1000 {
            let Some(_) = replicas.difference(&scanned).next() else {
                return observations;
            };
            assert!(
                tasks.iter().all(|task| !task.is_finished()),
                "watchdog failed before scanning"
            );
            let batch: Vec<Observation> = read_recall_events(store, cursor, 1000)
                .await
                .expect("read health facts")
                .into_iter()
                .map(|row| serde_json::from_value(json!(row)).expect("typed platform event"))
                .collect();
            cursor = batch.last().map_or(cursor, |row| row.storage_id);
            scanned.extend(batch.iter().filter_map(|row| {
                let PlatformEvent::Heartbeat {
                    replica_id,
                    store_bytes,
                } = row.payload.platform
                else {
                    return None;
                };
                assert!(store_bytes > 0, "a heartbeat reports the store's size");
                Some(replica_id)
            }));
            observations.extend(batch);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("watchdog exhausted scan completion attempts");
    })
    .await
    .expect("both replicas finish a paged scan without a scheduler")
}

pub(crate) async fn receive_webhook(
    State(receiver): State<WebhookReceiver>,
    headers: HeaderMap,
    Json(event): Json<PlatformEventEnvelope>,
) -> impl IntoResponse {
    assert_eq!(
        headers.get("Idempotency-Key").expect("deduplication key"),
        event.event_id.to_string().as_str()
    );
    let mut state = receiver.state.lock().expect("receiver state");
    state.requests.push(event);
    receiver.delivered.send_replace(state.requests.len());
    (
        StatusCode::from_u16(state.status).expect("configured response status"),
        [(axum::http::header::LOCATION, "/events")],
    )
}

/// Wait for `count` deliveries: fail after a minute without progress or ten minutes in total.
pub(crate) async fn wait_for_webhook(
    receiver: &WebhookReceiver,
    dispatcher: &JoinHandle<Result<(), ControlError>>,
    count: usize,
) {
    let mut delivered = receiver.delivered.subscribe();
    tokio::time::timeout(Duration::from_secs(600), async {
        for _ in 0..count {
            let seen = *delivered.borrow_and_update();
            tokio::time::timeout(
                Duration::from_secs(60),
                delivered.wait_for(|received| *received > seen || *received >= count),
            )
            .await
            .expect("dispatcher keeps delivering across bounded pages")
            .expect("webhook receiver remains available");
        }
    })
    .await
    .expect("dispatcher delivers every event within ten minutes");
    assert!(*delivered.borrow() >= count, "every event delivered");
    assert!(
        !dispatcher.is_finished(),
        "delivery loop stopped before draining"
    );
}

pub(crate) async fn publish_platform_impostor(store: &ControlStore, identity: (Uuid, Uuid)) {
    let mut requested = request(identity, 42, 0, &["orders"]);
    requested.image_id = None;
    queue(store, &requested).await;
    let lease = acquire_lease(store, (Uuid::now_v7(), None))
        .await
        .expect("elect")
        .expect("leader");
    let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
        .await
        .expect("claim")
        .expect("work");
    let batch = RecallBatch {
        invocation_id: "platform/impostor".into(),
        records: vec![RecallRecord {
            table: Table::Events,
            values: BTreeMap::from([
                (
                    "event_id".into(),
                    RecallValue::Text(Uuid::now_v7().to_string()),
                ),
                (
                    "event_type".into(),
                    RecallValue::Text("platform_heartbeat".into()),
                ),
                ("event_schema_version".into(), RecallValue::Integer(1)),
                ("event_kind".into(), RecallValue::Text("runtime".into())),
                ("severity".into(), RecallValue::Text("info".into())),
                (
                    "occurred_at".into(),
                    RecallValue::Text("2026-01-01T00:00:00Z".into()),
                ),
                ("producer".into(), RecallValue::Text("platform".into())),
                ("invocation_sequence".into(), RecallValue::Integer(0)),
                (
                    "payload".into(),
                    RecallValue::Text(
                        json!({"platform": {"kind": "heartbeat", "replica_id": Uuid::now_v7()}})
                            .to_string(),
                    ),
                ),
            ]),
        }],
    };
    let result = publish_run_recall(store, (claim.request.run_id, claim.claim_id), &batch)
        .await
        .expect("publish untrusted producer");
    assert_eq!(result.delivered, [(Table::Events, 1)]);
    assert!(result.conflicts.is_empty());
}

/// Every platform fact still stored after retention, in storage order.
pub(crate) async fn retained_platform_facts(store: &ControlStore) -> Vec<Observation> {
    read_recall_events(store, 0, 1000)
        .await
        .expect("read retained facts")
        .into_iter()
        .filter(|row| row["producer"] == "platform")
        .map(|row| serde_json::from_value(json!(row)).expect("typed platform event"))
        .collect()
}

/// The health history a test case describes, newest first as the server reads it, and the read time.
pub(crate) fn health_history(
    reported: &[Reported],
    later_reports: usize,
) -> (
    Vec<crate::platform::models::LocationHealthObservation>,
    chrono::DateTime<Utc>,
) {
    let now: chrono::DateTime<Utc> =
        chrono::DateTime::from_timestamp(1_795_000_000, 0).expect("a valid read time");
    // One session ID per instance: v7, stamped with when its process started, except the unordered one.
    let v7 = |started: chrono::DateTime<Utc>| -> Uuid {
        let seconds = u64::try_from(started.timestamp()).expect("a time after 1970");
        Uuid::new_v7(uuid::Timestamp::from_unix(uuid::NoContext, seconds, 0))
    };
    let sessions: [Uuid; 4] = [
        v7(now - TimeDelta::hours(3)),
        v7(now - TimeDelta::seconds(25)),
        v7(now - TimeDelta::days(8)),
        Uuid::from_u128(0x5_0000_0000_4000_8000_0000_0000_0000),
    ];
    let sessions: std::collections::BTreeMap<Instance, Uuid> = [
        Instance::Previous,
        Instance::Restarted,
        Instance::LongRunning,
        Instance::Unordered,
    ]
    .into_iter()
    .zip(sessions)
    .collect();
    let observation = |reported: &Reported| LocationHealthObservation {
        received_at: now - TimeDelta::seconds(reported.seconds_ago),
        report: LocationHealthReport {
            event_id: Uuid::now_v7(),
            session_id: sessions[&reported.instance],
            target: "production".into(),
            health: (reported.health.0)(reported.health.1),
        },
    };
    let later = (0..later_reports).map(|index| Reported {
        instance: Instance::Previous,
        seconds_ago: 20 - i64::try_from(index % 20).expect("a small index"),
        health: ReportedHealth(secrets, 0),
    });
    let mut observations: Vec<LocationHealthObservation> = reported
        .iter()
        .map(observation)
        .chain(later.map(|reported| observation(&reported)))
        .collect();
    observations.sort_by_key(|observation| std::cmp::Reverse(observation.received_at));
    observations.truncate(crate::platform::constants::RECENT_HEALTH_LIMIT);
    (observations, now)
}

/// One report in a health history.
pub(crate) const fn reported(
    instance: Instance,
    seconds_ago: i64,
    health: ReportedHealth,
) -> Reported {
    Reported {
        instance,
        seconds_ago,
        health,
    }
}

/// The instance's liveness row.
pub(crate) const fn check_in(_generation: i64) -> LocationHealth {
    LocationHealth::Heartbeat {}
}

/// A publication of the generation, then its healthy connection check.
pub(crate) const fn published(generation: i64) -> LocationHealth {
    LocationHealth::Connection {
        generation,
        healthy: true,
        trigger: None,
        elapsed_millis: None,
    }
}

/// A failed load of the generation.
pub(crate) fn failed(generation: i64) -> LocationHealth {
    LocationHealth::CompileFailed {
        generation,
        rules: None,
        error: Some(format!("generation {generation} failed to load")),
    }
}

/// The server's record that the plan of what changed on the generation failed.
pub(crate) fn change_plan_failed(generation: i64) -> LocationHealth {
    LocationHealth::ChangePlanFailed {
        generation,
        reason: "Model orders cannot plan a full rebuild".into(),
    }
}

/// A provider check without missing secrets.
pub(crate) const fn secrets(_generation: i64) -> LocationHealth {
    LocationHealth::ProviderSecrets {
        generation: 1,
        missing: 0,
        providers: Vec::new(),
    }
}

pub(crate) const UNREACHED: &str =
    "250001 Could not connect to Snowflake backend after 2 attempt(s)";

pub(crate) fn at(minutes: i64) -> chrono::DateTime<chrono::Utc> {
    "2026-11-25T18:29:00Z"
        .parse::<chrono::DateTime<chrono::Utc>>()
        .expect("time")
        + TimeDelta::minutes(minutes)
}

/// A sync report received at `minutes`, failed by `failure` with `read`, stopped or not.
pub(crate) fn sync(
    minutes: i64,
    failure: Option<crate::platform::models::WarehouseSyncFailure>,
    (read, halted): (Option<crate::platform::models::WarehouseReadFailure>, bool),
) -> crate::platform::models::LocationHealthObservation {
    crate::platform::models::LocationHealthObservation {
        received_at: at(minutes),
        report: LocationHealthReport {
            event_id: uuid::Uuid::now_v7(),
            session_id: uuid::Uuid::nil(),
            target: "prod".into(),
            health: crate::platform::models::LocationHealth::WarehouseSync {
                generation: 1,
                location: crate::warehouse_metadata::models::WarehouseStateLocation {
                    database: None,
                    schema: "ops".into(),
                },
                table: crate::warehouse_metadata::models::WarehouseHistoryKind::ALL[0],
                failure,
                records_sent: 0,
                elapsed_millis: 10,
                error: read.map(|_| UNREACHED.to_owned()),
                read_failure: read,
                page_limit: None,
                progress: Some(crate::platform::models::WarehouseSyncProgress {
                    position: 1,
                    kinds: 14,
                    rows: 0,
                    elapsed_millis: 10,
                    completed: false,
                    halted,
                }),
                next_sync_at: None,
                mirrored_records: None,
                failing_since: failure.map(|_| at(0)),
                retry_at: (failure.is_some() && !halted).then(|| at(minutes + 1)),
                retry_seconds: (failure.is_some() && !halted).then_some(60),
            },
        },
    }
}

/// The problem a failing sync of `orders` makes, stopped or not, reaching the warehouse or not.
pub(crate) fn failing_sync_of(
    halted: bool,
    unreachable: bool,
) -> crate::platform::models::FailingSync {
    crate::platform::models::FailingSync {
        location_id: uuid::Uuid::nil(),
        location_name: "orders".into(),
        target: "prod".into(),
        since: at(0),
        reason: UNREACHED.into(),
        unreachable,
        halted,
        retry_at: (!halted).then(|| at(4)),
        retry_seconds: (!halted).then_some(60),
        last_success_at: Some(at(-30)),
    }
}

/// A sink that accepted, dropped and failed the given numbers of events.
pub(crate) fn delivered(
    name: &str,
    (accepted, dropped, failed): (u64, u64, u64),
) -> crate::runs::models::SinkDelivery {
    crate::runs::models::SinkDelivery {
        name: name.into(),
        accepted,
        delivered: accepted - dropped - failed,
        dropped,
        failed,
    }
}

/// A run's sink report at `minutes`, from the `orders` location's `prod` target.
pub(crate) fn report(
    minutes: i64,
    sinks: Vec<crate::runs::models::SinkDelivery>,
    teardown_failures: Vec<String>,
) -> crate::platform::models::SinkReport {
    crate::platform::models::SinkReport {
        location_id: uuid::Uuid::nil(),
        location_name: "orders".into(),
        target: "prod".into(),
        run_id: Some(uuid::Uuid::from_u128(
            u128::try_from(minutes).unwrap_or_default(),
        )),
        at: at(minutes),
        sinks,
        teardown_failures,
    }
}

/// The problem `sink` of the newest report, at minute 3, makes for `reason`.
pub(crate) fn failing_sink_of(sink: &str, reason: &str) -> crate::platform::models::FailingSink {
    crate::platform::models::FailingSink {
        location_id: uuid::Uuid::nil(),
        location_name: "orders".into(),
        target: "prod".into(),
        sink: sink.into(),
        since: at(3),
        reason: reason.into(),
        run_id: Some(uuid::Uuid::from_u128(3)),
        last_success_at: None,
    }
}
