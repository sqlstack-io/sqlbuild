//! A tick an earlier release recorded in a form that no longer decodes, shown from the columns that always read.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::sensors::constants::{TICK_HANDLED, TICK_RUNNING, TICK_STARTED, UNREADABLE_TICK};
use crate::sensors::models::{
    PersonSensorCursor, PersonSensorEvaluation, PersonSensorRunRequest, SensorEvaluationOutcome,
    StoredEvaluationEvent,
};

/// The ticks newest first, with `events` put on them: an unreadable first phase matters only when no later one reads, a later one always.
pub(crate) fn with_unreadable(
    mut checks: BTreeMap<Uuid, PersonSensorEvaluation>,
    events: Vec<StoredEvaluationEvent>,
) -> Vec<PersonSensorEvaluation> {
    let requested: &str = SensorEvaluationOutcome::Requested.phase();
    let (first, later): (Vec<_>, Vec<_>) = events
        .into_iter()
        .partition(|event| event.phase == requested);
    for event in first {
        checks
            .entry(event.evaluation_id)
            .or_insert_with(|| unreadable_tick(&event));
    }
    for event in later {
        let check: &mut PersonSensorEvaluation = checks
            .entry(event.evaluation_id)
            .or_insert_with(|| unreadable_tick(&event));
        check.unreadable = Some(UNREADABLE_TICK.into());
        check.outcome = match event.phase.as_str() {
            "completed" => TICK_HANDLED.into(),
            phase => phase.into(),
        };
        check.phase = event.phase;
        check.finished_at = Some(event.occurred_at);
    }
    let mut ticks: Vec<PersonSensorEvaluation> = checks.into_values().collect();
    ticks.sort_by(|left, right| {
        (right.requested_at, right.evaluation_id).cmp(&(left.requested_at, left.evaluation_id))
    });
    ticks
}

/// What the columns say about a tick; its fingerprint is read on its own when the fact still has one.
fn unreadable_tick(event: &StoredEvaluationEvent) -> PersonSensorEvaluation {
    PersonSensorEvaluation {
        evaluation_id: event.evaluation_id,
        phase: SensorEvaluationOutcome::Requested.phase().into(),
        outcome: TICK_RUNNING.into(),
        requested_at: event.occurred_at,
        finished_at: None,
        cursor_before: PersonSensorCursor {
            event_storage_id: 0,
            custom: None,
        },
        cursor_after: None,
        event_id: event.subject_event_id.clone(),
        event_summary: None,
        run_ids: Vec::new(),
        skip_reason: None,
        failure_code: None,
        error: None,
        skipped_for: Vec::new(),
        abandoned_because: None,
        skipped_by: None,
        unreadable: Some(UNREADABLE_TICK.into()),
        definition_fingerprint: event.fact_json["request"]["definition_fingerprint"]
            .as_str()
            .unwrap_or_default()
            .into(),
    }
}

/// An unreadable completed tick started the runs that the recorded run requests name it for.
pub(crate) fn with_requested_runs(
    mut checks: Vec<PersonSensorEvaluation>,
    requests: &[PersonSensorRunRequest],
) -> Vec<PersonSensorEvaluation> {
    for check in checks.iter_mut().filter(|check| check.unreadable.is_some()) {
        check.run_ids = requests
            .iter()
            .filter(|request| request.evaluation_id == check.evaluation_id)
            .map(|request| request.run_id)
            .collect();
        if check.outcome == TICK_HANDLED && !check.run_ids.is_empty() {
            check.outcome = TICK_STARTED.into();
        }
    }
    checks
}
