use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::credentials::models::AuditContext;
use crate::declarations::main::change_trigger::change_trigger;
use crate::declarations::models::{TriggerChange, TriggerChangeRequest};
use crate::platform::models::PlatformEvent;
use crate::platform::models::{LocationWorkOutcome, LocationWorkResult};
use crate::runs::main::{
    execution::advance_groups::advance_groups, execution::change_run::change_run,
    execution::claim_next_run::claim_next_run, requests::request_job_run::request_job_run,
};
use crate::runs::models::{
    JobRunRequest, NodeOutcome, RunAuthority, RunChange, RunChangeRequest, RunCommand, RunKind,
    RunOutcome, RunRequest,
};
use crate::scheduler::models::SchedulerLease;
use crate::sensors::main::complete_sensor::complete_sensor;
use crate::sensors::models::{
    RequestedRun, SensorEvaluationFact, SensorEvaluationRequest, SensorFunctionOutcome,
};
use crate::snapshots::models::SnapshotInput;
use crate::store::main::append_control_event::append_control_event;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::query::query;
use crate::store::models::{ControlEvent, ControlStore, Parameter, TransactionMode};

use crate::runs::tests::helpers::snapshot as base_snapshot;
use crate::runs::tests::helpers::{json_column, rows};

/// Ticks a drain runs: more than any test has events, so each is delivered and the cursor reaches the end.
const DRAIN_TICKS: usize = 160;

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
                "description": "Refresh reporting on overdue ticks", "function": "start_orders", "code_hash": "v1", "timeout_seconds": 60,
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
            store_bytes: 0,
        },
        PlatformEvent::LocationBroken {
            location_id: Uuid::from_u128(9),
            target: "development".into(),
            last_seen_at: occurred_at,
        },
        PlatformEvent::Heartbeat {
            replica_id: Uuid::from_u128(10),
            store_bytes: 0,
        },
        PlatformEvent::LaunchFailed {
            run_id: Uuid::from_u128(11),
            claim_id: Uuid::from_u128(12),
            code: "local_spawn_failed".into(),
        },
        PlatformEvent::Heartbeat {
            replica_id: Uuid::from_u128(13),
            store_bytes: 0,
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
    let run_id = request_test_run(store, scope, job_key).await;
    finish_claimed_run(store, lease, scope, run_id, outcome, nodes, started).await;
    run_id
}

/// Queue a manual run of `job_key` as the test administrator.
async fn request_test_run(store: &ControlStore, scope: (Uuid, &str), job_key: &str) -> Uuid {
    let run_id = Uuid::now_v7();
    let context = AuditContext {
        actor: "test-admin".into(),
        granting_role: "control-store-owner".into(),
        request_id: Uuid::now_v7(),
        ip_address: None,
    };
    request_job_run(
        store,
        (&context, None),
        &JobRunRequest {
            run_id,
            location_id: scope.0,
            target: scope.1.into(),
            job_key: job_key.into(),
            reason: "Exercise terminal sensor scope".into(),
            priority: 10,
            window: None,
            full_refresh: false,
            select: Vec::new(),
            exclude: Vec::new(),
        },
    )
    .await
    .expect("request source run");
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
    finish_checked_run(
        store,
        lease,
        (scope, run_id),
        (outcome, nodes, started),
        &[],
    )
    .await;
}

/// Store a run as the ad-hoc run of `command` a worker would have executed.
pub(crate) async fn record_as_adhoc(store: &ControlStore, run_id: Uuid, command: RunCommand) {
    let stored = rows(
        store,
        "SELECT request_json FROM sqb_runs WHERE run_id=$1",
        vec![Parameter::Id(run_id)],
    )
    .await
    .remove(0);
    let mut request: RunRequest =
        serde_json::from_value(json_column(&stored["request_json"])).expect("stored request");
    request.kind = RunKind::Adhoc;
    request.job_key = None;
    request.command = command;
    let (transaction, _) = query(
        begin(store, TransactionMode::Write).await.expect("write"),
        "UPDATE sqb_runs SET kind='adhoc',job_key=NULL,request_json=$1 WHERE run_id=$2",
        vec![Parameter::Json(json!(request)), Parameter::Id(run_id)],
    )
    .await
    .expect("ad-hoc run");
    commit(transaction).await.expect("commit");
}

/// Request a run of `job_key` and finish it with `outcome`, its nodes succeeding and each attached check ending as given.
pub(crate) async fn finish_run_with_checks(
    store: &ControlStore,
    lease: &SchedulerLease,
    (scope, job_key): ((Uuid, &str), &str),
    outcome: RunOutcome,
    checks: &[(&str, NodeOutcome)],
) -> Uuid {
    let run_id = request_test_run(store, scope, job_key).await;
    finish_checked_run(
        store,
        lease,
        (scope, run_id),
        (outcome, [NodeOutcome::Success; 2], [true; 2]),
        checks,
    )
    .await;
    run_id
}

/// Finish a claimed run with its node outcomes, then each attached check's start and outcome.
async fn finish_checked_run(
    store: &ControlStore,
    lease: &SchedulerLease,
    (scope, run_id): ((Uuid, &str), Uuid),
    (outcome, nodes, started): (RunOutcome, [NodeOutcome; 2], [bool; 2]),
    checks: &[(&str, NodeOutcome)],
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
            table_rows: None,
            no_change_reason: None,
            error_code: None,
            skipped_because_of: None,
            error_message: None,
        });
    }
    for (check, finished) in checks {
        changes.push(RunChange::CheckStarted {
            node_key: (*check).into(),
        });
        changes.push(RunChange::CheckFinished {
            node_key: (*check).into(),
            outcome: *finished,
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

pub(crate) fn node_finished(node_key: &str, outcome: NodeOutcome) -> RunChange {
    RunChange::NodeFinished {
        node_key: node_key.into(),
        outcome,
        rows_affected: None,
        table_rows: None,
        no_change_reason: None,
        error_code: None,
        skipped_because_of: None,
        error_message: None,
    }
}

/// Evaluates each trigger whose evaluation is due and returns how many runs each one requested.
pub(crate) async fn evaluate_sensors(
    store: &ControlStore,
    lease: &SchedulerLease,
    triggers: &[crate::declarations::models::TriggerRecord],
) -> std::collections::BTreeMap<String, usize> {
    let mut fired = std::collections::BTreeMap::new();
    for trigger in triggers {
        let Some(requested) =
            crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation(
                store,
                lease,
                (trigger.trigger_id, None),
            )
            .await
            .expect("begin evaluation")
        else {
            continue;
        };
        let count = complete_starting_job(store, lease, &requested)
            .await
            .expect("complete evaluation");
        *fired.entry(trigger.name.clone()).or_default() += count;
    }
    fired
}

/// What a test sensor's function does, read from its name: `start_<job>` starts the job for the event.
pub(crate) fn stand_in_outcome(evaluation: &SensorEvaluationRequest) -> SensorFunctionOutcome {
    let Some(job) = evaluation.definition.function.strip_prefix("start_") else {
        return SensorFunctionOutcome::Handled { cursor: None };
    };
    let run_key = evaluation.event.as_ref().map_or_else(
        || evaluation.evaluation_id.to_string(),
        |event| format!("{}:{job}", event.id()),
    );
    SensorFunctionOutcome::Requested {
        cursor: None,
        runs: vec![RequestedRun {
            job: job.into(),
            run_key,
        }],
    }
}

/// Store `outcome` as the location's answer to the tick, as a location that ran the function would.
pub(crate) async fn answer_sensor(
    store: &ControlStore,
    evaluation: &SensorEvaluationRequest,
    outcome: SensorFunctionOutcome,
) {
    let result = LocationWorkResult {
        claim_id: Uuid::nil(),
        session_id: Uuid::nil(),
        outcome: LocationWorkOutcome::SensorEvaluation { outcome },
    };
    let (transaction, _) = query(
        begin(store, TransactionMode::Write).await.expect("answer"),
        "UPDATE sqb_location_work SET result_json=$2, completed_at=$3 WHERE request_id=$1 AND completed_at IS NULL",
        vec![
            Parameter::Id(evaluation.evaluation_id),
            Parameter::Json(json!(result)),
            Parameter::Time(Utc::now()),
        ],
    )
    .await
    .expect("store the location's answer");
    commit(transaction).await.expect("commit answer");
}

/// Answer the tick as the stand-in function named in its definition would, then complete it: the runs it started.
pub(crate) async fn complete_starting_job(
    store: &ControlStore,
    lease: &SchedulerLease,
    evaluation: &SensorEvaluationRequest,
) -> Result<usize, crate::store::errors::ControlError> {
    answer_sensor(store, evaluation, stand_in_outcome(evaluation)).await;
    complete_sensor(store, lease, evaluation).await
}

/// Run `DRAIN_TICKS` of the sensor's ticks, each event by its stand-in function: the runs they started.
pub(crate) async fn drain_sensor(
    store: &ControlStore,
    lease: &SchedulerLease,
    trigger_id: Uuid,
) -> usize {
    let mut started = 0;
    for _ in 0..DRAIN_TICKS {
        let requested = crate::sensors::main::begin_sensor_evaluation::begin_sensor_evaluation(
            store,
            lease,
            (trigger_id, None),
        )
        .await
        .expect("begin tick");
        started +=
            futures_util::future::OptionFuture::from(requested.map(|requested| async move {
                complete_starting_job(store, lease, &requested)
                    .await
                    .expect("complete tick")
            }))
            .await
            .unwrap_or_default();
    }
    started
}

/// Record a finished tick like `like`, requested `after` it, ending in `outcome`: the request it recorded.
pub(crate) async fn record_tick(
    store: &ControlStore,
    like: &SensorEvaluationRequest,
    after: chrono::TimeDelta,
    outcome: crate::sensors::models::SensorEvaluationOutcome,
) -> SensorEvaluationRequest {
    let request = SensorEvaluationRequest {
        evaluation_id: Uuid::now_v7(),
        requested_at: like.requested_at + after,
        ..like.clone()
    };
    let transaction = begin(store, TransactionMode::Write).await.expect("record");
    let transaction = crate::sensors::_helpers::ticks::facts::append_evaluation(
        transaction,
        &request,
        crate::sensors::models::SensorEvaluationOutcome::Requested,
    )
    .await
    .expect("requested");
    let transaction =
        crate::sensors::_helpers::ticks::facts::append_evaluation(transaction, &request, outcome)
            .await
            .expect("finished");
    commit(transaction).await.expect("commit tick");
    request
}

/// Add a sensor in the form servers stored before sensors were functions, as a pre-upgrade server would have published it: in the snapshot's `triggers` document and in `sqb_triggers`.
pub(crate) async fn add_old_form_sensor(
    store: &ControlStore,
    snapshot: &crate::snapshots::models::SnapshotRecord,
) {
    let definition = json!({
        "description": "Refresh reporting on overdue ticks", "job_key": "orders",
        "when": {"kind": "platform_event", "kinds": ["tick_overdue"]},
        "minimum_interval_seconds": 1
    });
    let (transaction, rows) = query(
        begin(store, TransactionMode::Write).await.expect("seed"),
        "SELECT d.body FROM sqb_snapshot_documents s JOIN sqb_documents d ON d.document_hash=s.document_hash \
         WHERE s.snapshot_id=$1 AND s.kind='triggers'",
        vec![Parameter::Id(snapshot.snapshot_id)],
    )
    .await
    .expect("read triggers document");
    let mut document: serde_json::Value = serde_json::from_slice(
        &crate::store::main::decompress::decompress(&rows[0]["body"]).expect("decompress"),
    )
    .expect("triggers document");
    document["health_orders"] = definition.clone();
    let (hash, body) =
        crate::snapshots::main::publication::stored_document::stored_document(&document)
            .expect("stored document");
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_documents (document_hash, body) VALUES ($1, $2)",
        vec![Parameter::Text(hash.clone()), Parameter::Bytes(body)],
    )
    .await
    .expect("store old-form document");
    let (transaction, _) = query(
        transaction,
        "UPDATE sqb_snapshot_documents SET document_hash=$2 WHERE snapshot_id=$1 AND kind='triggers'",
        vec![Parameter::Id(snapshot.snapshot_id), Parameter::Text(hash)],
    )
    .await
    .expect("point the snapshot at it");
    let (transaction, _) = query(
        transaction,
        "INSERT INTO sqb_triggers (trigger_id, location_id, target, kind, name, job_key, definition_json, snapshot_id) \
         VALUES ($1, $2, $3, 'sensor', 'health_orders', 'orders', $4, $5)",
        vec![
            Parameter::Id(Uuid::now_v7()),
            Parameter::Id(snapshot.location_id),
            Parameter::Text(snapshot.target.clone()),
            Parameter::Json(definition),
            Parameter::Id(snapshot.snapshot_id),
        ],
    )
    .await
    .expect("old-form trigger row");
    commit(transaction).await.expect("commit old-form sensor");
}

/// The `triggers` document hash of `snapshot_id`.
pub(crate) async fn triggers_document(store: &ControlStore, snapshot_id: Uuid) -> String {
    let (transaction, rows) = query(
        begin(store, TransactionMode::Read).await.expect("read"),
        "SELECT document_hash FROM sqb_snapshot_documents WHERE snapshot_id=$1 AND kind='triggers'",
        vec![Parameter::Id(snapshot_id)],
    )
    .await
    .expect("triggers document hash");
    commit(transaction).await.expect("commit read");
    rows[0]["document_hash"].as_str().expect("hash").to_owned()
}

/// The member's change of the sensor, or why it was refused.
pub(crate) async fn member_sensor_change(
    store: &ControlStore,
    who: (&AuditContext, Uuid),
    trigger_id: Uuid,
    change: TriggerChange,
) -> Result<bool, String> {
    crate::declarations::main::change_person_trigger::change_person_trigger(
        store,
        who,
        &TriggerChangeRequest {
            event_id: Uuid::now_v7(),
            trigger_id,
            actor: String::new(),
            reason: "Unblock the orders sensor".into(),
            change,
        },
    )
    .await
    .map_err(|error| {
        error
            .to_string()
            .trim_start_matches("control-plane operation denied: ")
            .split("; decision ")
            .next()
            .unwrap_or_default()
            .to_owned()
    })
}

/// A tick whose function returned `None`.
pub(crate) fn completed_tick() -> crate::sensors::models::SensorEvaluationOutcome {
    crate::sensors::models::SensorEvaluationOutcome::Completed {
        cursor: Default::default(),
        run_ids: Vec::new(),
        skipped: Vec::new(),
        skip_reason: None,
        skipped_by: None,
    }
}

/// A tick whose function raised.
pub(crate) fn failed_tick() -> crate::sensors::models::SensorEvaluationOutcome {
    crate::sensors::models::SensorEvaluationOutcome::Failed {
        code: "sensor_function_failed".into(),
        error: Some("ValueError: orders feed is down".into()),
    }
}

fn abandoned_tick(
    reason: crate::sensors::models::AbandonReason,
) -> crate::sensors::models::SensorEvaluationOutcome {
    crate::sensors::models::SensorEvaluationOutcome::Abandoned { reason }
}

/// A tick the location never answered.
pub(crate) fn unanswered_tick() -> crate::sensors::models::SensorEvaluationOutcome {
    abandoned_tick(crate::sensors::models::AbandonReason::Unanswered)
}

/// A tick cut short by a pause.
pub(crate) fn paused_tick() -> crate::sensors::models::SensorEvaluationOutcome {
    abandoned_tick(crate::sensors::models::AbandonReason::Paused)
}

/// A tick cut short by a new publication.
pub(crate) fn republished_tick() -> crate::sensors::models::SensorEvaluationOutcome {
    abandoned_tick(crate::sensors::models::AbandonReason::Republished)
}

/// A tick cut short by a cursor reset.
pub(crate) fn cursor_reset_tick() -> crate::sensors::models::SensorEvaluationOutcome {
    abandoned_tick(crate::sensors::models::AbandonReason::CursorReset)
}
