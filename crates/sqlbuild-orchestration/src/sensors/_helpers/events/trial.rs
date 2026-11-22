//! The published sensor and the events a trial chose, read in the caller's transaction; reading changes nothing.

use uuid::Uuid;

use crate::sensors::_helpers::events::conditions::recent_events;
use crate::sensors::_helpers::events::runs::run_event_for;
use crate::sensors::constants::{EVENT_PAGE_SIZE, MAX_TRIAL_EVENTS};
use crate::sensors::models::{
    EventScope, SensorCondition, SensorDefinition, SensorEvaluationFact, SensorEvent,
    SensorEventChoice, SensorTrialEvents,
};
use crate::store::errors::ControlError;
use crate::store::main::query::query;
use crate::store::models::{ControlTransaction, Parameter};

/// The trigger ID a sensor named `name` publishes under in the location's `target`.
#[must_use]
pub(crate) fn sensor_trigger_id(location_id: Uuid, target: &str, name: &str) -> Uuid {
    Uuid::new_v5(
        &location_id,
        &serde_json::to_vec(&(target, "sensor", name)).unwrap_or_default(),
    )
}

/// The published sensor with the events `choice` picks, or why there are none to pick.
pub(crate) async fn sensor_events(
    transaction: ControlTransaction,
    ((location_id, target), (name, choice)): ((Uuid, &str), (&str, &SensorEventChoice)),
    origin: Option<&str>,
) -> Result<(ControlTransaction, Result<SensorTrialEvents, String>), ControlError> {
    let trigger_id = sensor_trigger_id(location_id, target, name);
    let (transaction, rows) = query(
        transaction,
        "SELECT t.definition_json, t.snapshot_id FROM sqb_triggers t WHERE t.trigger_id=$1 \
         AND t.kind='sensor' AND t.removed_in_snapshot_id IS NULL",
        vec![Parameter::Id(trigger_id)],
    )
    .await?;
    let Some(row) = rows.first() else {
        return Ok((
            transaction,
            Err(format!(
                "sensor {name} isn't published on target {target}; check the name, or let the location publish it"
            )),
        ));
    };
    let definition: SensorDefinition = serde_json::from_value(row["definition_json"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let snapshot_id: Uuid = serde_json::from_value(row["snapshot_id"].clone())
        .map_err(|error| ControlError::Invalid(error.to_string()))?;
    let scope = EventScope {
        trigger_id,
        location_id,
        target,
        snapshot_id,
        origin,
    };
    let trial = |events: Vec<SensorEvent>| SensorTrialEvents {
        sensor: name.into(),
        function: definition.function.clone(),
        when: definition.when.clone(),
        timeout_seconds: definition.timeout_seconds.get(),
        events,
    };
    let Some(condition) = &definition.when else {
        return Ok((
            transaction,
            match choice {
                SensorEventChoice::Recent { .. } | SensorEventChoice::Latest => {
                    Ok(trial(Vec::new()))
                }
                _ => Err(format!(
                    "sensor {name} polls instead of reacting to events, so --event and --run don't apply; use --latest to call it once"
                )),
            },
        ));
    };
    let (transaction, chosen) = match choice {
        SensorEventChoice::Recent { limit } => {
            let limit = usize::try_from(*limit)
                .unwrap_or(MAX_TRIAL_EVENTS)
                .clamp(1, MAX_TRIAL_EVENTS);
            let (transaction, events) =
                recent_events(transaction, scope, (condition, limit)).await?;
            (transaction, Ok(events))
        }
        SensorEventChoice::Latest => {
            let (transaction, events) = recent_events(transaction, scope, (condition, 1)).await?;
            (transaction, Ok(events))
        }
        SensorEventChoice::Event { event_id } => {
            let (transaction, event) = event_by_id(transaction, scope, condition, event_id).await?;
            (
                transaction,
                event.map(|event| vec![event]).ok_or_else(|| {
                    format!(
                        "no event {event_id} among sensor {name}'s ticks or the recent events its condition matches"
                    )
                }),
            )
        }
        SensorEventChoice::Run { run_id } => {
            let SensorCondition::RunEvent {
                job_key,
                kinds,
                executed_successfully,
            } = condition
            else {
                return Ok((
                    transaction,
                    Err(format!(
                        "sensor {name} doesn't watch run_event, so --run doesn't apply; use --event or --latest"
                    )),
                ));
            };
            let (transaction, event) = run_event_for(
                transaction,
                scope,
                *run_id,
                (job_key, kinds, executed_successfully.as_ref()),
            )
            .await?;
            (transaction, event.map(|event| vec![event]))
        }
    };
    Ok((transaction, chosen.map(trial)))
}

/// The event `event_id` as the sensor's newest tick with it received it, else among the recent matching events.
async fn event_by_id(
    transaction: ControlTransaction,
    scope: EventScope<'_>,
    condition: &SensorCondition,
    event_id: &str,
) -> Result<(ControlTransaction, Option<SensorEvent>), ControlError> {
    let (transaction, rows) = query(
        transaction,
        "SELECT fact_json FROM sqb_sensor_evaluation_events WHERE trigger_id=$1 \
         AND subject_event_id=$2 AND phase='requested' ORDER BY occurred_at DESC LIMIT 1",
        vec![
            Parameter::Id(scope.trigger_id),
            Parameter::Text(event_id.into()),
        ],
    )
    .await?;
    if let Some(row) = rows.first() {
        let fact: SensorEvaluationFact = serde_json::from_value(row["fact_json"].clone())
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        return Ok((transaction, fact.request.event));
    }
    let (transaction, events) =
        recent_events(transaction, scope, (condition, EVENT_PAGE_SIZE)).await?;
    Ok((
        transaction,
        events.into_iter().find(|event| event.id() == event_id),
    ))
}
