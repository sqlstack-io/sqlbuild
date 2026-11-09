//! A person sees a sensor's cursor, recent checks and run requests only with view on every current node.

use std::collections::BTreeMap;

use sqlbuild_core::redaction::main::redact_text::redact_text;
use uuid::Uuid;

use crate::authorization::main::authorize_listing::authorize_listing;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::sensors::_helpers::facts::definition_fingerprint;
use crate::sensors::models::{
    PersonSensorCursor, PersonSensorEvaluation, PersonSensorRunRequest, PersonSensorState,
    SensorCursor, SensorEvaluationOutcome, StoredEvaluationEvent, StoredRunRequest, StoredSensor,
};
use crate::store::errors::ControlError;
use crate::store::main::{begin::begin, commit::commit, query_records::query_records};
use crate::store::models::{ControlStore, ControlTransaction, Parameter, TransactionMode};

/// `None` when no sensor by that ID is in this location and target; authorization runs first either way.
pub async fn read_person_sensor(
    store: &ControlStore,
    (context, credentials): (&AuditContext, Uuid),
    ((location_id, target), (trigger_id, limit)): ((Uuid, &str), (Uuid, u32)),
    default_role: DefaultRole,
) -> Result<Option<PersonSensorState>, ControlError> {
    let (transaction, _) = authorize_listing(
        begin(store, TransactionMode::Write).await?,
        (context, credentials),
        (location_id, target),
        default_role,
    )
    .await?;
    let (transaction, sensors): (_, Vec<StoredSensor>) = query_records(
        transaction,
        "SELECT t.definition_json, s.cursor_json FROM sqb_triggers t \
         LEFT JOIN sqb_trigger_state s ON s.trigger_id=t.trigger_id \
         WHERE t.trigger_id=$1 AND t.location_id=$2 AND t.target=$3 AND t.kind='sensor'",
        vec![
            Parameter::Id(trigger_id),
            Parameter::Id(location_id),
            Parameter::Text(target.into()),
        ],
    )
    .await?;
    let Some(sensor) = sensors.into_iter().next() else {
        commit(transaction).await?;
        return Ok(None);
    };
    let limit = Parameter::Integer(i64::from(limit.min(1000)));
    let (transaction, evaluations) = evaluations(transaction, trigger_id, limit.clone()).await?;
    let (transaction, requests): (_, Vec<StoredRunRequest>) = query_records(
        transaction,
        "SELECT q.run_key, q.run_id, q.evaluation_id, r.queued_at, r.status FROM sqb_run_requests q \
         JOIN sqb_runs r ON r.run_id=q.run_id WHERE q.trigger_id=$1 \
         ORDER BY r.queued_at DESC, q.run_key LIMIT $2",
        vec![Parameter::Id(trigger_id), limit],
    )
    .await?;
    commit(transaction).await?;
    Ok(Some(PersonSensorState {
        cursor: sensor.cursor_json.as_ref().map(shown_cursor),
        definition_fingerprint: definition_fingerprint(&sensor.definition_json)?,
        evaluations,
        run_requests: requests
            .into_iter()
            .map(|request| PersonSensorRunRequest {
                run_key: redact_text(&request.run_key),
                run_id: request.run_id,
                evaluation_id: request.evaluation_id,
                queued_at: request.queued_at,
                status: request.status,
            })
            .collect(),
    }))
}

/// The newest `limit` checks, each projected from its recorded phases.
async fn evaluations(
    transaction: ControlTransaction,
    trigger_id: Uuid,
    limit: Parameter,
) -> Result<(ControlTransaction, Vec<PersonSensorEvaluation>), ControlError> {
    let (transaction, events): (_, Vec<StoredEvaluationEvent>) = query_records(
        transaction,
        "SELECT occurred_at, fact_json FROM sqb_sensor_evaluation_events WHERE trigger_id=$1 \
         AND evaluation_id IN (SELECT evaluation_id FROM sqb_sensor_evaluation_events \
         WHERE trigger_id=$1 AND phase='requested' ORDER BY occurred_at DESC LIMIT $2)",
        vec![Parameter::Id(trigger_id), limit],
    )
    .await?;
    let mut checks: BTreeMap<Uuid, PersonSensorEvaluation> = BTreeMap::new();
    for event in events {
        let request = &event.fact_json.request;
        let check = checks
            .entry(request.evaluation_id)
            .or_insert_with(|| PersonSensorEvaluation {
                evaluation_id: request.evaluation_id,
                phase: SensorEvaluationOutcome::Requested.phase().into(),
                requested_at: request.requested_at,
                finished_at: None,
                cursor_before: shown_cursor(&request.cursor),
                cursor_after: None,
                run_count: 0,
                failure_code: None,
                definition_fingerprint: request.definition_fingerprint.clone(),
            });
        let outcome = event.fact_json.outcome;
        if outcome == SensorEvaluationOutcome::Requested {
            continue;
        }
        check.phase = outcome.phase().into();
        check.finished_at = Some(event.occurred_at);
        match outcome {
            SensorEvaluationOutcome::Completed { cursor, run_ids } => {
                check.cursor_after = Some(shown_cursor(&cursor));
                check.run_count = run_ids.len();
            }
            SensorEvaluationOutcome::Failed { code } => check.failure_code = Some(code),
            _ => {}
        }
    }
    let mut checks: Vec<PersonSensorEvaluation> = checks.into_values().collect();
    checks.sort_by(|left, right| {
        (right.requested_at, right.evaluation_id).cmp(&(left.requested_at, left.evaluation_id))
    });
    Ok((transaction, checks))
}

fn shown_cursor(cursor: &SensorCursor) -> PersonSensorCursor {
    PersonSensorCursor {
        event_storage_id: cursor.event_storage_id,
        custom: cursor.custom.as_deref().map(redact_text),
    }
}
