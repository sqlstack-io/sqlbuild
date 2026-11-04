//! Trigger and failure fixtures exercise the scheduler's transactional tick boundary.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use chrono::{DateTime, DurationRound, SecondsFormat, TimeDelta, Timelike, Utc};

use crate::declarations::main::list_triggers::list_triggers;
use crate::runs::main::list_runs::list_runs;
use crate::scheduler::main::_acquire_lease::acquire_lease;
use crate::scheduler::main::_evaluate_trigger::evaluate_trigger;
use crate::scheduler::main::_scheduler_cycle::scheduler_cycle;
use crate::scheduler::main::list_ticks::list_ticks;
use crate::scheduler::main::run_scheduler::run_scheduler;
use crate::scheduler::models::{SchedulerLease, SchedulerState, TickOutcome, TickRecord};
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::snapshots::models::{SnapshotInput, SnapshotNode};
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::{begin::begin, commit::commit, query::query};
use crate::store::models::{ControlStore, Parameter, TransactionMode};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::scheduler::tests::contracts::ticks::test_types::{
    FaultAnomalies, FaultStep, IdleLauncher, TransactionFault,
};
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::helpers::read_recall_events;
use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store};
use crate::store::tests::store_test_types::Backend;

/// Calendar fixtures stay inside the tick-retention horizon: day 0 is midnight three days ago.
pub(crate) fn calendar_time(day: i64) -> DateTime<Utc> {
    static BASE: LazyLock<DateTime<Utc>> = LazyLock::new(|| {
        (Utc::now() - TimeDelta::days(3))
            .duration_trunc(TimeDelta::days(1))
            .expect("calendar base")
    });
    *BASE + TimeDelta::days(day)
}

pub(crate) fn calendar_text(day: i64) -> String {
    calendar_time(day).to_rfc3339_opts(SecondsFormat::Secs, true)
}

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
                json!({"daily_orders": {"description": "Refresh orders", "job_key": "orders", "starts_at": calendar_text(0),
                "schedule": {"kind": "calendar", "times": [calendar_text(0), calendar_text(1), calendar_text(2), "2030-01-01T00:00:00Z"], "refresh_at": "2029-01-01T00:00:00Z"}}}),
            ),
        ]),
    }
}

async fn trigger_audit_rows(store: &ControlStore) -> Vec<(String, String, String, String, String)> {
    let sql = "SELECT CAST(event_id AS TEXT), actor, granting_role, CAST(before_json AS TEXT), CAST(after_json AS TEXT) FROM sqb_security_audit WHERE action='trigger.changed'";
    let ControlStore::Postgres(pool) = store else {
        let ControlStore::Sqlite(path, _) = store else {
            unreachable!()
        };
        return rusqlite::Connection::open(path)
            .expect("inspect trigger audit")
            .prepare(sql)
            .expect("audit query")
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })
            .expect("read trigger audits")
            .collect::<Result<_, _>>()
            .expect("audit rows");
    };
    sqlx_core::query_as::query_as(sql)
        .fetch_all(pool)
        .await
        .expect("trigger audits")
}

pub(crate) async fn trigger_audits(store: &ControlStore) -> Vec<Value> {
    trigger_audit_rows(store)
        .await
        .into_iter()
        .map(|(event_id, actor, granting_role, before, after)| {
            json!({
                "event_id": event_id,
                "actor": actor,
                "granting_role": granting_role,
                "before_json": serde_json::from_str::<Value>(&before).expect("before state"),
                "after_json": serde_json::from_str::<Value>(&after).expect("after state"),
            })
        })
        .collect()
}

/// Crash points inside the tick transaction, from the tick row to the cursor and the fence.
pub(crate) fn tick_faults() -> [TransactionFault; 7] {
    [
        TransactionFault {
            description: "tick insertion",
            table: "sqb_ticks",
            operation: "INSERT",
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT,'injected tick failure')",
            postgres_body: "RAISE EXCEPTION 'injected tick failure'",
        },
        TransactionFault {
            description: "requested event",
            table: "recall_events",
            operation: "INSERT",
            condition: "WHEN NEW.event_type='run_requested'",
            sqlite_body: "SELECT RAISE(ABORT,'injected requested failure')",
            postgres_body: "RAISE EXCEPTION 'injected requested failure'",
        },
        TransactionFault {
            description: "queued event",
            table: "recall_events",
            operation: "INSERT",
            condition: "WHEN NEW.event_type='run_queued'",
            sqlite_body: "SELECT RAISE(ABORT,'injected queued failure')",
            postgres_body: "RAISE EXCEPTION 'injected queued failure'",
        },
        TransactionFault {
            description: "run projection",
            table: "sqb_runs",
            operation: "INSERT",
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT,'injected run failure')",
            postgres_body: "RAISE EXCEPTION 'injected run failure'",
        },
        TransactionFault {
            description: "node projection",
            table: "sqb_run_nodes",
            operation: "INSERT",
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT,'injected node failure')",
            postgres_body: "RAISE EXCEPTION 'injected node failure'",
        },
        TransactionFault {
            description: "cursor commit",
            table: "sqb_trigger_state",
            operation: "UPDATE",
            condition: "",
            sqlite_body: "SELECT RAISE(ABORT,'injected cursor failure')",
            postgres_body: "RAISE EXCEPTION 'injected cursor failure'",
        },
        TransactionFault {
            description: "lease expires during append",
            table: "sqb_runs",
            operation: "INSERT",
            condition: "",
            sqlite_body: "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
            postgres_body: "UPDATE sqb_scheduler_lease SET expires_at='2000-01-01T00:00:00.000000Z'",
        },
    ]
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
    for point in tick_faults() {
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

/// Test-only clock skew: move a trigger's past calendar, cursor and ticks back by `days`.
pub(crate) async fn shift_schedule(store: &ControlStore, trigger_id: Uuid, days: i64) {
    let shift = |value: &mut Value| {
        let time: DateTime<Utc> = serde_json::from_value(value.clone()).expect("calendar time");
        *value = json!(time - TimeDelta::days(days));
    };
    let past = |value: &&mut Value| {
        serde_json::from_value::<DateTime<Utc>>((*value).clone()).expect("calendar time")
            < Utc::now()
    };
    let transaction = begin(store, TransactionMode::Write).await.expect("shift");
    let (transaction, rows) = query(
        transaction,
        "SELECT definition_json FROM sqb_triggers WHERE trigger_id=$1",
        vec![Parameter::Id(trigger_id)],
    )
    .await
    .expect("trigger definition");
    let mut definition = rows[0]["definition_json"].clone();
    shift(&mut definition["starts_at"]);
    for time in definition["schedule"]["times"]
        .as_array_mut()
        .expect("calendar times")
        .iter_mut()
        .filter(past)
    {
        shift(time);
    }
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_triggers SET definition_json=$1 WHERE trigger_id=$2",
        vec![Parameter::Json(definition), Parameter::Id(trigger_id)],
    )
    .await
    .expect("shift calendar");
    let (transaction, ticks) = query(
        transaction,
        "SELECT tick_id, scheduled_for FROM sqb_ticks WHERE trigger_id=$1",
        vec![Parameter::Id(trigger_id)],
    )
    .await
    .expect("ticks");
    let mut transaction = transaction;
    for tick in ticks {
        let mut time = tick["scheduled_for"].clone();
        shift(&mut time);
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_ticks SET scheduled_for=$1 WHERE tick_id=$2",
            vec![
                Parameter::Time(serde_json::from_value(time).expect("tick time")),
                Parameter::Id(serde_json::from_value(tick["tick_id"].clone()).expect("tick")),
            ],
        )
        .await
        .expect("shift tick");
    }
    let (transaction, state) = query(
        transaction,
        "SELECT started_at, last_tick_for FROM sqb_trigger_state WHERE trigger_id=$1",
        vec![Parameter::Id(trigger_id)],
    )
    .await
    .expect("trigger state");
    let started: DateTime<Utc> =
        serde_json::from_value(state[0]["started_at"].clone()).expect("start");
    let last: Option<DateTime<Utc>> =
        serde_json::from_value(state[0]["last_tick_for"].clone()).expect("cursor");
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_trigger_state SET started_at=$1, last_tick_for=$2 WHERE trigger_id=$3",
        vec![
            Parameter::Time(started - TimeDelta::days(days)),
            Parameter::OptionalTime(last.map(|time| time - TimeDelta::days(days))),
            Parameter::Id(trigger_id),
        ],
    )
    .await
    .expect("shift state");
    commit(transaction).await.expect("commit shift");
}

/// A later covered tick, as if another due time arrived while the requested run still waited.
pub(crate) async fn append_covered_tick(
    store: &ControlStore,
    trigger_id: Uuid,
    scheduled_for: DateTime<Utc>,
) {
    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("covered tick");
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_ticks (tick_id, trigger_id, scheduled_for, evaluated_at, epoch, outcome, \
         run_id, covered_by_run_id, details_json) SELECT $1, trigger_id, $2, evaluated_at, epoch, \
         'covered', NULL, run_id, details_json FROM sqb_ticks WHERE trigger_id=$3 AND outcome='requested'",
        vec![
            Parameter::Id(Uuid::now_v7()),
            Parameter::Time(scheduled_for),
            Parameter::Id(trigger_id),
        ],
    )
    .await
    .expect("append covered tick");
    commit(transaction).await.expect("commit covered tick");
}

/// Rewrite one trigger-state column the way an earlier build or a past start would have left it.
pub(crate) async fn set_trigger_state(
    store: &ControlStore,
    trigger_id: Uuid,
    sql: &str,
    time: DateTime<Utc>,
) {
    let transaction = begin(store, TransactionMode::Write).await.expect("state");
    let (transaction, _) = query(
        transaction,
        sql,
        vec![Parameter::Time(time), Parameter::Id(trigger_id)],
    )
    .await
    .expect("rewrite trigger state");
    commit(transaction).await.expect("commit state");
}

pub(crate) async fn tick_times(store: &ControlStore, trigger_id: Uuid) -> Vec<DateTime<Utc>> {
    list_ticks(store, trigger_id, 1000)
        .await
        .expect("ticks")
        .into_iter()
        .map(|tick| tick.scheduled_for)
        .collect()
}

/// Deterministic splitmix64 choices, so a failing seed replays exactly.
pub(crate) fn draw(state: &mut u64, bound: u64) -> usize {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    usize::try_from((value ^ (value >> 31)) % bound).expect("choice fits usize")
}

pub(crate) fn draw_step(state: &mut u64) -> FaultStep {
    FaultStep {
        restart: [None, None, None, None, Some(0), Some(1)][draw(state, 6)],
        expire: draw(state, 4) == 0,
        fault: draw(state, 14).min(7),
        cycles: [[true, false], [false, true], [true, true]][draw(state, 3)],
    }
}

/// One process cycle; an error is that process crashing and restarting with a new holder.
async fn cycle_or_restart(store: &ControlStore, state: SchedulerState) -> (SchedulerState, usize) {
    let result = scheduler_cycle(store, state).await;
    let crashed = usize::from(result.is_err());
    (
        result.map_or_else(|_| SchedulerState::new(Uuid::now_v7()), |(state, _)| state),
        crashed,
    )
}

/// Seeded random faults across two schedulers, then recovery; returns contract breaches and step counts.
pub(crate) async fn fault_run(
    backend: Backend,
    seed: u64,
    steps: usize,
) -> Option<(FaultAnomalies, BTreeMap<&'static str, usize>)> {
    let fixture = open_test_store(backend).await?;
    let store = &fixture.store;
    migrate_store(store).await.expect("migrate");
    let location = create_location(store, "orders", "test-admin")
        .await
        .expect("location");
    let mut input = snapshot(location);
    input.documents.insert(
        "jobs".into(),
        json!({"orders": {"description": "Build orders", "selection": ["orders"], "overlap": "queue"}}),
    );
    input.documents.insert(
        "triggers".into(),
        json!({"minutely_orders": {"description": "Minutely orders", "job_key": "orders",
            "starts_at": "2026-01-01T00:00:00Z", "catch_up": {"all": {"max_runs": 3}},
            "schedule": {"kind": "cron", "expression": "* * * * *", "timezone": "UTC"}}}),
    );
    publish_snapshot(store, &input).await.expect("publication");
    let trigger_id = list_triggers(store, location, "development")
        .await
        .expect("trigger")[0]
        .trigger_id;
    let started = Utc::now()
        .duration_trunc(TimeDelta::minutes(1))
        .expect("minute")
        - TimeDelta::minutes(20)
        + TimeDelta::milliseconds(250);
    set_trigger_state(store, trigger_id,
        "UPDATE sqb_trigger_state SET started_at=$1, last_tick_for=NULL, next_due_at=$1 WHERE trigger_id=$2",
        started).await;
    let second = second_store(store).await;
    let handles = [store, &second];
    let mut states = [
        SchedulerState::new(Uuid::now_v7()),
        SchedulerState::new(Uuid::now_v7()),
    ];
    let faults: Vec<TransactionFault> = tick_faults()
        .into_iter()
        .chain([TransactionFault {
            description: "no fault",
            table: "sqb_ticks",
            operation: "INSERT",
            condition: "WHEN 1=0",
            sqlite_body: "SELECT 1",
            postgres_body: "PERFORM 1",
        }])
        .collect();
    let mut choices = seed;
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut anomalies = FaultAnomalies::default();
    for _ in 0..steps {
        let step = draw_step(&mut choices);
        step.restart
            .into_iter()
            .for_each(|index| states[index] = SchedulerState::new(Uuid::now_v7()));
        // A lapsed lease leaves every current holder frozen on an epoch that must now be fenced.
        let stale: Vec<SchedulerLease> = states
            .iter()
            .filter_map(|state| state.lease.clone())
            .filter(|_| step.expire)
            .collect();
        for _ in 0..usize::from(step.expire) {
            expire_lease(store).await;
        }
        let fault = &faults[step.fault];
        install_fault(store, fault).await;
        let cycle = |index: usize| {
            let (handle, mut state, cycles) =
                (handles[index], states[index].clone(), step.cycles[index]);
            async move {
                let mut crashed = 0;
                for _ in 0..usize::from(cycles) {
                    (state, crashed) = cycle_or_restart(handle, state).await;
                }
                (state, crashed)
            }
        };
        let ((left, left_crashed), (right, right_crashed)) = tokio::join!(cycle(0), cycle(1));
        states = [left, right];
        remove_fault(store, fault).await;
        for lease in &stale {
            anomalies.unfenced_stale_writes += usize::from(!matches!(
                evaluate_trigger(store, lease, trigger_id).await,
                Err(ControlError::Fenced)
            ));
        }
        for (name, count) in [
            ("restarts", usize::from(step.restart.is_some())),
            ("lease expiries", usize::from(step.expire)),
            ("armed faults", usize::from(step.fault < 7)),
            ("crashes", left_crashed + right_crashed),
            (
                "concurrent cycles",
                usize::from(step.cycles == [true, true]),
            ),
            ("frozen leases fenced", stale.len()),
        ] {
            *counts.entry(name).or_default() += count;
        }
    }
    // Recovery: faults are gone and leadership lapses; one survivor drains whatever is still due.
    expire_lease(store).await;
    let mut survivor = SchedulerState::new(Uuid::now_v7());
    for _ in 0..20 {
        survivor = scheduler_cycle(&second, survivor)
            .await
            .expect("recovery")
            .0;
    }
    let settled = Utc::now();
    scheduler_cycle(&second, survivor)
        .await
        .expect("final recovery");
    let ticks = list_ticks(store, trigger_id, 1000).await.expect("ticks");
    let times: BTreeSet<DateTime<Utc>> = ticks.iter().map(|tick| tick.scheduled_for).collect();
    let first = started
        .duration_trunc(TimeDelta::minutes(1))
        .expect("minute")
        + TimeDelta::minutes(1);
    let expected: BTreeSet<DateTime<Utc>> = (0..)
        .map(|minutes| first + TimeDelta::minutes(minutes))
        .take_while(|time| *time <= settled)
        .collect();
    anomalies.duplicate_ticks = ticks.len() - times.len();
    anomalies.missing_ticks = expected.difference(&times).count();
    anomalies.off_schedule_ticks = times
        .iter()
        .filter(|time| **time < first || time.second() != 0 || time.nanosecond() != 0)
        .count();
    let runs: BTreeSet<Uuid> = list_runs(store, (location, "development"), 1000)
        .await
        .expect("runs")
        .iter()
        .map(|run| run.run_id)
        .collect();
    let tick_runs: BTreeSet<Uuid> = ticks.iter().filter_map(|tick| tick.run_id).collect();
    anomalies.runs_without_exactly_one_tick = runs.symmetric_difference(&tick_runs).count()
        + ticks.iter().filter(|tick| tick.run_id.is_none()).count();
    let requested = read_recall_events(store, 0, 10_000)
        .await
        .expect("events")
        .iter()
        .filter(|event| event["event_type"] == "run_requested")
        .count();
    anomalies.request_event_mismatch = requested.abs_diff(ticks.len());
    let transaction = begin(store, TransactionMode::Write)
        .await
        .expect("duplicate");
    let duplicate = query(transaction,
        "INSERT INTO sqb_ticks (tick_id, trigger_id, scheduled_for, evaluated_at, epoch, outcome, run_id, covered_by_run_id, details_json) \
         VALUES ($1,$2,$3,$3,1,'skipped',NULL,NULL,$4)",
        vec![Parameter::Id(Uuid::now_v7()), Parameter::Id(trigger_id), Parameter::Time(first), Parameter::Json(json!({}))]).await;
    anomalies.accepted_duplicate_ticks = usize::from(duplicate.is_ok());
    counts.insert("ticks", ticks.len());
    close_store(second).await;
    close_test_store(fixture).await;
    Some((anomalies, counts))
}

/// Run one real scheduler loop until `deadline`, then stop it and return the trigger's ticks.
pub(crate) async fn run_scheduler_until(
    store: &ControlStore,
    trigger_id: Uuid,
    deadline: DateTime<Utc>,
) -> Vec<TickRecord> {
    let (stop, shutdown) = tokio::sync::watch::channel(false);
    let (_changed, changes) = tokio::sync::watch::channel(());
    let stopping = async {
        tokio::time::sleep((deadline - Utc::now()).to_std().unwrap_or_default()).await;
        stop.send(true).expect("stop scheduler");
    };
    let (finished, ()) = tokio::join!(
        run_scheduler(
            store,
            SchedulerState::new(Uuid::now_v7()),
            (shutdown, changes),
            IdleLauncher
        ),
        stopping
    );
    finished.expect("scheduler loop");
    list_ticks(store, trigger_id, 100).await.expect("ticks")
}

pub(crate) async fn queued_times(
    store: &ControlStore,
    location: Uuid,
) -> BTreeMap<Uuid, DateTime<Utc>> {
    list_runs(store, (location, "development"), 100)
        .await
        .expect("runs")
        .into_iter()
        .map(|run| (run.run_id, run.queued_at))
        .collect()
}
