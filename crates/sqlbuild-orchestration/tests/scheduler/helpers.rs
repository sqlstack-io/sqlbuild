//! Contention and expiry exercise scheduler ownership against real databases, not fake clocks.

pub(crate) use crate::scheduler::main::_acquire_lease::acquire_lease;
pub(crate) use crate::scheduler::main::_evaluate_trigger::evaluate_trigger;
pub(crate) use crate::scheduler::main::_scheduler_cycle::scheduler_cycle;
use crate::scheduler::main::renew_lease::renew_lease;
use crate::scheduler::models::SchedulerLease;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::models::ControlStore;
use futures_util::{StreamExt, stream};
use uuid::Uuid;

use crate::store::tests::lease_helpers::expire_lease;
use crate::store::tests::replica_helpers::second_store;
use crate::store::tests::store_helpers::{close_store, close_test_store, open_test_store};
use crate::store::tests::store_test_types::Backend;

/// Renew `lease` every five seconds, as a running scheduler does, until a renewal fails.
pub(crate) fn hold_lease(
    store: &ControlStore,
    lease: &SchedulerLease,
) -> tokio::task::JoinHandle<()> {
    let store = store.clone();
    let renewals = stream::try_unfold(lease.clone(), move |lease| {
        let store = store.clone();
        async move {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            let started = std::time::Instant::now();
            renew_lease(&store, &lease)
                .await
                .map(|renewed| Some(((), renewed)))
                .inspect_err(|error| {
                    println!(
                        "lease holder stopped: renewal failed after {:?} with {error}; the lease was due to expire at {}",
                        started.elapsed(),
                        lease.expires_at
                    );
                })
        }
    });
    tokio::spawn(renewals.for_each(|_| async {}))
}

pub(crate) async fn leadership_contract(backend: Backend) -> Option<i64> {
    let test_store = open_test_store(backend).await?;
    let store = &test_store.store;
    migrate_store(store)
        .await
        .expect("migrate leadership store");
    let second = second_store(store).await;
    let (first, other) = tokio::join!(
        acquire_lease(store, Uuid::now_v7()),
        acquire_lease(&second, Uuid::now_v7())
    );
    let first = first.expect("first contender completes");
    let other = other.expect("second contender completes");
    assert_ne!(
        first.is_some(),
        other.is_some(),
        "exactly one concurrent contender is elected"
    );
    let lease = first.or(other).expect("one elected holder");
    let renewed = renew_lease(&second, &lease)
        .await
        .expect("live epoch renews");
    assert_eq!(renewed.epoch, lease.epoch);
    assert!(renewed.expires_at >= lease.expires_at);
    assert!(
        acquire_lease(store, lease.holder)
            .await
            .expect("duplicate acquisition completes")
            .is_none()
    );
    let mut forged = lease.clone();
    forged.holder = Uuid::now_v7();
    assert!(matches!(
        renew_lease(store, &forged).await,
        Err(ControlError::Fenced)
    ));
    expire_lease(store).await;
    assert!(
        matches!(renew_lease(store, &lease).await, Err(ControlError::Fenced)),
        "even an uncontested expired holder must reacquire a new epoch"
    );
    let successor = acquire_lease(&second, Uuid::now_v7())
        .await
        .expect("acquire expired lease")
        .expect("successor elected");
    assert!(matches!(
        renew_lease(store, &lease).await,
        Err(ControlError::Fenced)
    ));
    renew_lease(store, &successor)
        .await
        .expect("successor owns the current fence");
    let epoch = successor.epoch;
    close_store(second).await;
    close_test_store(test_store).await;
    Some(epoch)
}

/// The operator a test's manual run requests come from.
pub(crate) fn operator() -> crate::credentials::models::AuditContext {
    crate::credentials::models::AuditContext {
        actor: "test-operator".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    }
}

/// A manual request for one run of `job_key` on the development target.
pub(crate) fn manual_run(location_id: Uuid, job_key: &str) -> crate::runs::models::JobRunRequest {
    crate::runs::models::JobRunRequest {
        run_id: Uuid::now_v7(),
        location_id,
        target: "development".into(),
        job_key: job_key.into(),
        reason: "Build it now".into(),
        priority: 0,
        window: None,
        full_refresh: false,
        select: Vec::new(),
        exclude: Vec::new(),
    }
}

/// Write every published job's definition back with the concurrency source an older release recorded.
pub(crate) async fn store_old_job_definitions(store: &ControlStore, source: &str) {
    use crate::runs::tests::helpers::{json_column, rows};
    use crate::store::main::{begin::begin, commit::commit, query::query};
    use crate::store::models::{Parameter, TransactionMode};

    let jobs = rows(
        store,
        "SELECT job_key, definition_json FROM sqb_jobs",
        vec![],
    )
    .await;
    let mut transaction = begin(store, TransactionMode::Write).await.expect("begin");
    for job in jobs {
        let mut definition: serde_json::Value = json_column(&job["definition_json"]);
        definition["resolved_concurrency"]["source"] = serde_json::json!(source);
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_jobs SET definition_json=$1 WHERE job_key=$2",
            vec![
                Parameter::Json(definition),
                Parameter::Text(job["job_key"].as_str().expect("job key").to_owned()),
            ],
        )
        .await
        .expect("store the old definition");
    }
    commit(transaction).await.expect("commit");
}

/// Point each snapshot's `jobs` and `groups` documents at old-form bodies under hashes of their own, as an older release stored them.
pub(crate) async fn store_old_documents(store: &ControlStore, source: &str) {
    use crate::runs::tests::helpers::rows;
    use crate::store::main::{
        begin::begin, commit::commit, compress::compress, decompress::decompress, query::query,
    };
    use crate::store::models::{Parameter, TransactionMode};

    let documents = rows(
        store,
        "SELECT b.document_hash, b.body FROM sqb_snapshot_documents d \
         JOIN sqb_documents b ON b.document_hash=d.document_hash WHERE d.kind IN ('jobs', 'groups')",
        vec![],
    )
    .await;
    let mut transaction = begin(store, TransactionMode::Write).await.expect("begin");
    for document in documents {
        let mut body: serde_json::Value =
            serde_json::from_slice(&decompress(&document["body"]).expect("body")).expect("JSON");
        for job in body.as_object_mut().expect("jobs or groups").values_mut() {
            job["resolved_concurrency"]["source"] = serde_json::json!(source);
        }
        let hash = document["document_hash"].as_str().expect("hash").to_owned();
        let old_hash = format!("{hash}-published-before-0004");
        (transaction, _) = query(
            transaction,
            "INSERT INTO sqb_documents (document_hash, body) VALUES ($1, $2)",
            vec![
                Parameter::Text(old_hash.clone()),
                Parameter::Bytes(
                    compress(&serde_json::to_vec(&body).expect("JSON")).expect("gzip"),
                ),
            ],
        )
        .await
        .expect("store the old document");
        (transaction, _) = query(
            transaction,
            "UPDATE sqb_snapshot_documents SET document_hash=$1 WHERE document_hash=$2",
            vec![Parameter::Text(old_hash), Parameter::Text(hash)],
        )
        .await
        .expect("point the snapshots at it");
    }
    commit(transaction).await.expect("commit");
}

/// A control-store result as a test compares it: `ok`, or the error as `Invalid("...")` and the like.
pub(crate) fn outcome<T>(result: Result<T, ControlError>) -> String {
    result.map_or_else(|error| format!("{error:?}"), |_| "ok".to_owned())
}

/// The scheduled orders snapshot with job `orders`, a group `nightly_group` of it and `triggers`, as a release before 0004 published it once its source is stored.
pub(crate) fn upgrade_snapshot(
    location_id: Uuid,
    triggers: serde_json::Value,
) -> crate::snapshots::models::SnapshotInput {
    let mut input = crate::scheduler::tests::contracts::ticks::helpers::snapshot(location_id);
    input.documents.insert(
        "jobs".into(),
        serde_json::json!({"orders": {"description": "Build orders", "selection": ["orders"],
            "resolved_concurrency": {"limit": 4}}}),
    );
    input.documents.insert(
        "groups".into(),
        serde_json::json!({"nightly_group": {"description": "Everything nightly",
            "selection": ["orders"], "jobs": {"orders": []}, "resolved_concurrency": {"limit": 4}}}),
    );
    input.documents.insert("triggers".into(), triggers);
    input
}

/// Report a claimed run as its worker does: started, each node with `node`, then finished with `run`; each report's outcome.
pub(crate) async fn report_run(
    store: &ControlStore,
    claim: &crate::runs::models::RunClaim,
    (node, run): (
        crate::runs::models::NodeOutcome,
        crate::runs::models::RunOutcome,
    ),
) -> Vec<String> {
    use crate::runs::models::RunChange;

    let mut changes = vec![RunChange::Started];
    changes.extend(
        claim
            .request
            .selection
            .iter()
            .map(|node_key| RunChange::NodeFinished {
                node_key: node_key.clone(),
                outcome: node,
                rows_affected: Some(1),
                error_code: None,
                skipped_because_of: None,
                error_message: None,
            }),
    );
    changes.push(RunChange::Finished {
        outcome: run,
        reason: "the worker finished".into(),
        exit_code: Some(1),
    });
    report_changes(store, claim, changes).await
}

/// Report `changes` for a claimed run as its worker does; each report's outcome.
pub(crate) async fn report_changes(
    store: &ControlStore,
    claim: &crate::runs::models::RunClaim,
    changes: Vec<crate::runs::models::RunChange>,
) -> Vec<String> {
    use crate::runs::main::execution::change_run::change_run;
    use crate::runs::models::{RunAuthority, RunChangeRequest};

    let mut outcomes = Vec::new();
    for change in changes {
        let reported = change_run(
            store,
            RunAuthority::Worker {
                run_id: claim.request.run_id,
                claim_id: claim.claim_id,
            },
            &RunChangeRequest {
                event_id: Uuid::now_v7(),
                run_id: claim.request.run_id,
                claim_id: Some(claim.claim_id),
                change,
            },
        )
        .await;
        outcomes.push(outcome(reported));
    }
    outcomes
}

/// A control-store result as a test compares it: `ok`, or the error as people read it.
pub(crate) fn refusal<T>(result: Result<T, ControlError>) -> String {
    result.map_or_else(|error| error.to_string(), |_| "ok".to_owned())
}

/// Every run as `job status: reason`, by job then run.
pub(crate) async fn run_states(store: &ControlStore) -> Vec<String> {
    crate::runs::tests::helpers::rows(
        store,
        "SELECT job_key, status, status_reason FROM sqb_runs ORDER BY job_key, run_id",
        vec![],
    )
    .await
    .iter()
    .map(|row| {
        format!(
            "{} {}: {}",
            row["job_key"].as_str().unwrap_or_default(),
            row["status"].as_str().unwrap_or_default(),
            row["status_reason"].as_str().unwrap_or_default()
        )
    })
    .collect()
}

/// The live problems `principal_id` can see.
pub(crate) async fn problems(
    store: &ControlStore,
    principal_id: uuid::Uuid,
) -> crate::platform::models::PlatformProblems {
    crate::platform::main::person_reads::read_person_problems::read_person_problems(
        store,
        principal_id,
        crate::credentials::models::DefaultRole::None,
    )
    .await
    .expect("problems")
}

/// The triggers the live problems list for `principal_id`: skipped ones as `name: reason`, then sensors checked again from scratch by name.
pub(crate) async fn trigger_problems(
    store: &ControlStore,
    principal_id: uuid::Uuid,
) -> (Vec<String>, Vec<String>) {
    let problems = problems(store, principal_id).await;
    (
        problems
            .skipped_triggers
            .into_iter()
            .map(|trigger| format!("{}: {}", trigger.name, trigger.reason))
            .collect(),
        problems
            .rechecked_sensors
            .into_iter()
            .map(|sensor| sensor.name)
            .collect(),
    )
}

/// The triggers the live problems list as skipped for `principal_id`, as `name: reason`.
pub(crate) async fn skipped_triggers(
    store: &ControlStore,
    principal_id: uuid::Uuid,
) -> Vec<String> {
    trigger_problems(store, principal_id).await.0
}

/// The calendars whose dropped times the live problems list for `principal_id`, as `name: time, time`.
pub(crate) async fn dropped_calendar_times(
    store: &ControlStore,
    principal_id: uuid::Uuid,
) -> Vec<String> {
    problems(store, principal_id)
        .await
        .dropped_calendar_times
        .into_iter()
        .map(|calendar| {
            let times: Vec<String> = calendar
                .times
                .iter()
                .map(|time| time.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                .collect();
            format!("{}: {}", calendar.name, times.join(", "))
        })
        .collect()
}

/// Publish sensor `health_orders` in `location_id`, let it check once, apply `earlier_form`, then make it due again with a new overdue tick to see; the scheduler's state and the sensor.
pub(crate) async fn sensor_due_again(
    store: &ControlStore,
    location_id: uuid::Uuid,
    earlier_form: &str,
) -> (crate::scheduler::models::SchedulerState, uuid::Uuid) {
    let mut input = upgrade_snapshot(
        location_id,
        serde_json::json!({"health_orders": {"description": "Refresh orders on overdue ticks",
            "job_key": "orders",
            "when": {"kind": "platform_event", "kinds": ["tick_overdue"]},
            "minimum_interval_seconds": 1}}),
    );
    input.generation = 2;
    crate::snapshots::main::publication::publish_snapshot::publish_snapshot(store, &input)
        .await
        .expect("publish");
    crate::sensors::tests::helpers::append_events(store, 0..1, chrono::Utc::now()).await;
    let (state, _) = scheduler_cycle(
        store,
        crate::scheduler::models::SchedulerState::new(uuid::Uuid::now_v7()),
    )
    .await
    .expect("the sensor's first check");
    execute(store, earlier_form).await;
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    crate::sensors::tests::helpers::append_events(store, 6..7, chrono::Utc::now()).await;
    execute(
        store,
        "UPDATE sqb_trigger_state SET next_due_at='2000-01-01T00:00:00Z' \
         WHERE trigger_id IN (SELECT trigger_id FROM sqb_triggers WHERE kind='sensor')",
    )
    .await;
    let trigger_id = serde_json::from_value(
        crate::runs::tests::helpers::rows(
            store,
            "SELECT trigger_id FROM sqb_triggers WHERE kind='sensor'",
            vec![],
        )
        .await[0]["trigger_id"]
            .clone(),
    )
    .expect("sensor ID");
    (state, trigger_id)
}

/// Give `snapshot_id` a `jobs` document in the form an older release published, which no longer decodes.
pub(crate) async fn store_unreadable_jobs(store: &ControlStore, snapshot_id: uuid::Uuid) {
    use crate::store::main::{begin::begin, commit::commit, compress::compress, query::query};
    use crate::store::models::{Parameter, TransactionMode};

    let body = serde_json::json!({"orders": {"description": "Build orders", "selection": ["orders"],
        "resolved_concurrency": {"limit": 4, "source": "project"}}});
    let hash = format!("{snapshot_id}-jobs-published-before-0004");
    let transaction = begin(store, TransactionMode::Write).await.expect("begin");
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_documents (document_hash, body) VALUES ($1, $2)",
        vec![
            Parameter::Text(hash.clone()),
            Parameter::Bytes(compress(&serde_json::to_vec(&body).expect("JSON")).expect("gzip")),
        ],
    )
    .await
    .expect("store the old document");
    let (transaction, _) = query(
        transaction,
        "DELETE FROM sqb_snapshot_documents WHERE snapshot_id=$1 AND kind='jobs'",
        vec![Parameter::Id(snapshot_id)],
    )
    .await
    .expect("drop the current jobs document");
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_snapshot_documents (snapshot_id, kind, document_hash) VALUES ($1, 'jobs', $2)",
        vec![Parameter::Id(snapshot_id), Parameter::Text(hash)],
    )
    .await
    .expect("point the snapshot at it");
    commit(transaction).await.expect("commit");
}

/// Two jobs, the second waiting for the first, and the group that runs them.
pub(crate) fn group_snapshot(location: uuid::Uuid) -> crate::snapshots::models::SnapshotInput {
    let mut input = crate::runs::tests::helpers::snapshot(location);
    input.nodes.get_mut("orders").expect("orders").definition["deps"] =
        serde_json::json!(["customers"]);
    input.documents.insert(
        "jobs".into(),
        serde_json::json!({
            "staging_job": {"description": "Stage customers", "selection": ["customers"],
                "resolved_concurrency": {"limit": 4}},
            "marts_job": {"description": "Build orders", "selection": ["orders"],
                "resolved_concurrency": {"limit": 4}}
        }),
    );
    input.documents.insert(
        "groups".into(),
        serde_json::json!({"nightly_job_group": {
            "description": "Stage then build orders", "selection": ["customers", "orders"],
            "jobs": {"staging_job": [], "marts_job": ["staging_job"]}
        }}),
    );
    input
}

/// Execute `sql` against the store as it stands, such as to put a stored row in the form an earlier release wrote it.
pub(crate) async fn execute(store: &ControlStore, sql: &str) {
    use crate::store::main::{begin::begin, commit::commit, query::query};
    use crate::store::models::TransactionMode;

    let transaction = begin(store, TransactionMode::Write).await.expect("begin");
    let (transaction, _) = query(transaction, sql, vec![]).await.expect(sql);
    commit(transaction).await.expect("commit");
}

/// A nightly cron schedule of `orders`.
pub(crate) fn nightly_cron() -> serde_json::Value {
    serde_json::json!({"nightly_orders": {"description": "Refresh orders nightly", "job_key": "orders",
        "schedule": {"kind": "cron", "expression": "0 0 * * *", "timezone": "UTC"}}})
}

/// A calendar schedule of `orders` at `days`, then far in the future.
pub(crate) fn nightly_calendar(days: &[i64]) -> serde_json::Value {
    let times: Vec<String> = days
        .iter()
        .map(|day| crate::scheduler::tests::contracts::ticks::helpers::calendar_text(*day))
        .chain(["2030-01-01T00:00:00Z".to_owned()])
        .collect();
    serde_json::json!({"nightly_orders": {"description": "Refresh orders on business days", "job_key": "orders",
        "starts_at": crate::scheduler::tests::contracts::ticks::helpers::calendar_text(0),
        "schedule": {"kind": "calendar", "times": times, "refresh_at": "2029-01-01T00:00:00Z"}}})
}

/// The calendar times days 0 to 2 that the republish drops, as the live problems list them.
pub(crate) fn dropped_days_problem() -> String {
    format!(
        "nightly_orders: {}, {}, {}",
        crate::scheduler::tests::contracts::ticks::helpers::calendar_text(0),
        crate::scheduler::tests::contracts::ticks::helpers::calendar_text(1),
        crate::scheduler::tests::contracts::ticks::helpers::calendar_text(2)
    )
}
