use std::collections::BTreeMap;

use serde_json::json;
use sqlbuild_orchestration::declarations::main::list_triggers::list_triggers;
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::scheduler::main::acquire_lease::acquire_lease;
use sqlbuild_orchestration::scheduler::main::evaluate_trigger::evaluate_trigger;
use sqlbuild_orchestration::scheduler::main::list_ticks::list_ticks;
use sqlbuild_orchestration::scheduler::models::TickOutcome;
use sqlbuild_orchestration::snapshots::main::publish_snapshot::publish_snapshot;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotNode};
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::create_location::create_location;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::read_recall_events::read_recall_events;
use uuid::Uuid;

use crate::fault_helpers::{install_fault, remove_fault};
use crate::lease_helpers::expire_lease;
use crate::replica_helpers::second_store;
use crate::store_helpers::{close_store, close_test_store, open_test_store};
use crate::store_test_types::Backend;
use crate::test_types::TickFaultTestCase;

pub(crate) fn snapshot(location_id: Uuid) -> SnapshotInput {
    SnapshotInput {
        location_id,
        target: "development".into(),
        generation: 1,
        git_revision: None,
        git_dirty: false,
        git_author: None,
        image_id: Some("sha256:synthetic-orders-image".into()),
        environment_hash: "orders-environment".into(),
        sqb_version: env!("CARGO_PKG_VERSION").into(),
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
            (
                "triggers".into(),
                json!({"daily_orders": {"description": "Refresh orders", "job_key": "orders", "starts_at": "2020-01-01T00:00:00Z",
                "schedule": {"kind": "calendar", "times": ["2020-01-01T00:00:00Z", "2020-01-02T00:00:00Z", "2020-01-03T00:00:00Z", "2030-01-01T00:00:00Z"], "refresh_at": "2029-01-01T00:00:00Z"}}}),
            ),
        ]),
    }
}

pub(crate) async fn transaction_contract(backend: Backend) -> Option<(usize, usize)> {
    let fixture = open_test_store(backend).await?;
    let store = &fixture.store;
    migrate_store(store).await.expect("migrate tick store");
    let location = create_location(store, "orders", "test-admin")
        .await
        .expect("create location");
    publish_snapshot(store, &snapshot(location))
        .await
        .expect("publish job and trigger");
    let trigger_id = list_triggers(store, location, "development")
        .await
        .expect("published triggers")[0]
        .trigger_id;
    let lease = acquire_lease(store, Uuid::now_v7())
        .await
        .expect("acquire lease")
        .expect("elected");
    let second = second_store(store).await;
    for point in [
        TickFaultTestCase {
            description: "tick insertion",
            table: "sqb_ticks",
            operation: "INSERT",
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT,'injected tick failure')",
            postgres_body: "RAISE EXCEPTION 'injected tick failure'",
        },
        TickFaultTestCase {
            description: "requested event",
            table: "recall_events",
            operation: "INSERT",
            condition: "WHEN NEW.event_type='run_requested'",
            sqlite_body: "SELECT RAISE(ABORT,'injected requested failure')",
            postgres_body: "RAISE EXCEPTION 'injected requested failure'",
        },
        TickFaultTestCase {
            description: "queued event",
            table: "recall_events",
            operation: "INSERT",
            condition: "WHEN NEW.event_type='run_queued'",
            sqlite_body: "SELECT RAISE(ABORT,'injected queued failure')",
            postgres_body: "RAISE EXCEPTION 'injected queued failure'",
        },
        TickFaultTestCase {
            description: "run projection",
            table: "sqb_runs",
            operation: "INSERT",
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT,'injected run failure')",
            postgres_body: "RAISE EXCEPTION 'injected run failure'",
        },
        TickFaultTestCase {
            description: "node projection",
            table: "sqb_run_nodes",
            operation: "INSERT",
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT,'injected node failure')",
            postgres_body: "RAISE EXCEPTION 'injected node failure'",
        },
        TickFaultTestCase {
            description: "cursor commit",
            table: "sqb_trigger_state",
            operation: "UPDATE",
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT,'injected cursor failure')",
            postgres_body: "RAISE EXCEPTION 'injected cursor failure'",
        },
        TickFaultTestCase {
            description: "lease expires during append",
            table: "sqb_runs",
            operation: "INSERT",
            condition: "",
            sqlite_body: "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
            postgres_body: "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
        },
    ] {
        install_fault(store, &point).await;
        let failed = evaluate_trigger(store, &lease, trigger_id).await;
        assert!(failed.is_err(), "{} must abort", point.description);
        remove_fault(store, &point).await;
        assert!(
            list_ticks(&second, trigger_id, 100)
                .await
                .expect("rolled-back ticks")
                .is_empty(),
            "{}",
            point.description
        );
        assert!(
            list_runs(&second, (location, "development"), 100)
                .await
                .expect("rolled-back runs")
                .is_empty(),
            "{}",
            point.description
        );
        assert!(
            read_recall_events(&second, 0, 100)
                .await
                .expect("rolled-back events")
                .is_empty(),
            "{}",
            point.description
        );
        assert!(
            list_triggers(&second, location, "development")
                .await
                .expect("rolled-back cursor")[0]
                .last_tick_for
                .is_none(),
            "{}",
            point.description
        );
    }
    let (left, right) = tokio::join!(
        evaluate_trigger(store, &lease, trigger_id),
        evaluate_trigger(&second, &lease, trigger_id)
    );
    let first_count = left
        .expect("first scheduler")
        .expect("active trigger")
        .actions
        .len();
    let second_count = right
        .expect("second scheduler")
        .expect("active trigger")
        .actions
        .len();
    assert_eq!(
        first_count + second_count,
        3,
        "restarts never repeat committed ticks"
    );
    let ticks = list_ticks(store, trigger_id, 100)
        .await
        .expect("committed ticks");
    let runs = list_runs(store, (location, "development"), 100)
        .await
        .expect("committed run");
    let events = read_recall_events(store, 0, 100)
        .await
        .expect("committed events");
    assert_eq!(
        events
            .iter()
            .map(|row| row["event_type"].as_str().expect("event type"))
            .collect::<Vec<_>>(),
        ["run_requested", "run_queued"]
    );
    assert_eq!(ticks[0].outcome, TickOutcome::Requested);
    assert!(
        ticks[1..]
            .iter()
            .all(|tick| tick.outcome == TickOutcome::Covered
                && tick.covered_by_run_id == Some(runs[0].run_id))
    );
    assert_eq!(
        runs[0].last_event_storage_id,
        events[1]["storage_id"].as_i64().expect("event cursor")
    );
    expire_lease(store).await;
    let successor = acquire_lease(&second, Uuid::now_v7())
        .await
        .expect("takeover")
        .expect("new leader");
    assert!(matches!(
        evaluate_trigger(store, &lease, trigger_id).await,
        Err(ControlError::Fenced)
    ));
    assert!(
        evaluate_trigger(&second, &successor, trigger_id)
            .await
            .expect("failover catch-up")
            .expect("active trigger")
            .actions
            .is_empty()
    );
    let counts = (ticks.len(), runs.len());
    close_store(second).await;
    close_test_store(fixture).await;
    Some(counts)
}
