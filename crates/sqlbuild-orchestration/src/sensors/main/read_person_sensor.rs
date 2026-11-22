//! A person sees a sensor's cursor, recent ticks and run requests only with view on every current node; what the function wrote also needs view.evidence.

use std::collections::BTreeMap;

use sqlbuild_core::redaction::main::redact_text::redact_text;
use uuid::Uuid;

use crate::authorization::main::decisions::authorize_listing::authorize_listing;
use crate::authorization::main::decisions::person_holds_target::person_holds_target;
use crate::credentials::models::{Action, AuditContext, DefaultRole};
use crate::sensors::_helpers::ticks::facts::definition_fingerprint;
use crate::sensors::_helpers::ticks::unreadable::{with_requested_runs, with_unreadable};
use crate::sensors::main::sensor_failing::sensor_failing;
use crate::sensors::models::{
    PersonSensorCursor, PersonSensorEvaluation, PersonSensorRunRequest, PersonSensorState,
    SensorCursor, SensorEvaluationFact, SensorEvaluationOutcome, SensorEvent,
    StoredEvaluationEvent, StoredRunRequest, StoredSensor,
};

use crate::sensors::constants::{TICK_HANDLED, TICK_RUNNING, TICK_SKIPPED, TICK_STARTED};
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
    let evidence = person_holds_target(
        store,
        credentials,
        (Action::ViewEvidence, location_id, target),
        default_role,
    )
    .await?;
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
    let (transaction, evaluations) =
        evaluations(transaction, (trigger_id, limit.clone()), evidence).await?;
    let (transaction, requests): (_, Vec<StoredRunRequest>) = query_records(
        transaction,
        "SELECT q.run_key, q.run_id, q.evaluation_id, r.queued_at, r.status FROM sqb_run_requests q \
         JOIN sqb_runs r ON r.run_id=q.run_id WHERE q.trigger_id=$1 \
         ORDER BY r.queued_at DESC, q.run_key LIMIT $2",
        vec![Parameter::Id(trigger_id), limit],
    )
    .await?;
    let (transaction, failing) = sensor_failing(transaction, trigger_id).await?;
    commit(transaction).await?;
    let run_requests: Vec<PersonSensorRunRequest> = requests
        .into_iter()
        .map(|request| PersonSensorRunRequest {
            run_key: if evidence {
                redact_text(&request.run_key)
            } else {
                String::new()
            },
            run_id: request.run_id,
            evaluation_id: request.evaluation_id,
            queued_at: request.queued_at,
            status: request.status,
        })
        .collect();
    Ok(Some(PersonSensorState {
        failing,
        cursor: sensor
            .cursor_json
            .as_ref()
            .map(|cursor| shown_cursor(cursor, evidence)),
        definition_fingerprint: definition_fingerprint(&sensor.definition_json)?,
        evaluations: with_requested_runs(evaluations, &run_requests),
        run_requests,
    }))
}

/// The newest `limit` ticks, each projected from its recorded phases; one that no longer decodes loses only its own details.
async fn evaluations(
    transaction: ControlTransaction,
    (trigger_id, limit): (Uuid, Parameter),
    evidence: bool,
) -> Result<(ControlTransaction, Vec<PersonSensorEvaluation>), ControlError> {
    let (transaction, events): (_, Vec<StoredEvaluationEvent>) = query_records(
        transaction,
        "SELECT evaluation_id, phase, occurred_at, subject_event_id, fact_json FROM sqb_sensor_evaluation_events WHERE trigger_id=$1 \
         AND evaluation_id IN (SELECT evaluation_id FROM sqb_sensor_evaluation_events \
         WHERE trigger_id=$1 AND phase='requested' ORDER BY occurred_at DESC LIMIT $2)",
        vec![Parameter::Id(trigger_id), limit],
    )
    .await?;
    let mut checks: BTreeMap<Uuid, PersonSensorEvaluation> = BTreeMap::new();
    let mut unreadable: Vec<StoredEvaluationEvent> = Vec::new();
    for event in events {
        let Ok(fact) = serde_json::from_value::<SensorEvaluationFact>(event.fact_json.clone())
        else {
            unreadable.push(event);
            continue;
        };
        let request = &fact.request;
        let check = checks
            .entry(request.evaluation_id)
            .or_insert_with(|| PersonSensorEvaluation {
                evaluation_id: request.evaluation_id,
                phase: SensorEvaluationOutcome::Requested.phase().into(),
                outcome: TICK_RUNNING.into(),
                requested_at: request.requested_at,
                finished_at: None,
                cursor_before: shown_cursor(&request.cursor, evidence),
                cursor_after: None,
                event_id: request.event.as_ref().map(|event| event.id().to_owned()),
                event_summary: request.event.as_ref().map(SensorEvent::summary),
                run_ids: Vec::new(),
                skip_reason: None,
                failure_code: None,
                error: None,
                skipped_for: Vec::new(),
                abandoned_because: None,
                skipped_by: None,
                unreadable: None,
                definition_fingerprint: request.definition_fingerprint.clone(),
            });
        let outcome = fact.outcome;
        if outcome == SensorEvaluationOutcome::Requested {
            continue;
        }
        check.phase = outcome.phase().into();
        check.outcome = outcome.phase().into();
        check.finished_at = Some(event.occurred_at);
        match outcome {
            SensorEvaluationOutcome::Completed {
                cursor,
                run_ids,
                skipped,
                skip_reason,
                skipped_by,
            } => {
                check.skipped_by = skipped_by;
                check.cursor_after = Some(shown_cursor(&cursor, evidence));
                check.outcome = if !run_ids.is_empty() || !skipped.is_empty() {
                    TICK_STARTED
                } else if skip_reason.is_some() {
                    TICK_SKIPPED
                } else {
                    TICK_HANDLED
                }
                .into();
                check.run_ids = run_ids;
                check.skipped_for = skipped.iter().map(|skip| skip.active_run_id).collect();
                check.skip_reason = skip_reason
                    .filter(|_| evidence)
                    .map(|reason| redact_text(&reason));
            }
            SensorEvaluationOutcome::Failed { code, error } => {
                check.failure_code = Some(code);
                check.error = error.filter(|_| evidence).map(|error| redact_text(&error));
            }
            SensorEvaluationOutcome::Abandoned { reason } => {
                check.abandoned_because = Some(reason);
            }
            _ => {}
        }
    }
    Ok((transaction, with_unreadable(checks, unreadable)))
}

/// A custom value is what the sensor computed from data it read, so it shows only with view.evidence.
fn shown_cursor(cursor: &SensorCursor, evidence: bool) -> PersonSensorCursor {
    PersonSensorCursor {
        event_storage_id: cursor.event_storage_id,
        custom: cursor
            .custom
            .as_deref()
            .filter(|_| evidence)
            .map(redact_text),
    }
}
