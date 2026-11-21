//! What `sqb sensor try` prints: the events to choose from, the warning before a real call, and what the function did.

use std::fmt::Write as _;

use sqlbuild_orchestration::sensors::models::{
    SensorCondition, SensorEvent, SensorFunctionOutcome, SensorTrialEvents,
};

/// The listing of a bare `try`: each event's ID, time and summary, then how to choose one.
pub(crate) fn event_listing(events: &SensorTrialEvents) -> String {
    let name = &events.sensor;
    let Some(condition) = &events.when else {
        return format!(
            "{name} polls instead of reacting to events.\nCall it once with: sqb sensor try {name} --latest\n"
        );
    };
    if events.events.is_empty() {
        return format!("No recent events match {name}'s condition.\n");
    }
    let mut text = String::new();
    let width = events
        .events
        .iter()
        .map(|event| event.id().len())
        .max()
        .unwrap_or_default();
    for event in &events.events {
        let _ = writeln!(
            text,
            "{:width$}  {}  {}",
            event.id(),
            event.occurred_at().format("%Y-%m-%d %H:%M:%S UTC"),
            event.summary()
        );
    }
    let run = if matches!(condition, SensorCondition::RunEvent { .. }) {
        ", --run <run-id>"
    } else {
        ""
    };
    let _ = writeln!(
        text,
        "\nCall {name} with one: sqb sensor try {name} --event <id> (or --latest{run})"
    );
    text
}

/// The line before the call: the function runs for real, with the target's providers.
pub(crate) fn call_warning(
    events: &SensorTrialEvents,
    (event, target): (Option<&SensorEvent>, &str),
) -> String {
    let with = event.map_or_else(
        || "once, as a poll".to_owned(),
        |event| format!("with {} ({})", event.id(), event.summary()),
    );
    format!(
        "Calling {}() for sensor {} {with}, using target {target}'s providers.\nWhat they do, such as sending a message, really happens; no run starts and no tick is recorded.\n",
        events.function, events.sensor
    )
}

/// What the function did, and the exit status: `0` when it returned, `1` when it failed or timed out.
pub(crate) fn outcome_report(
    outcome: &SensorFunctionOutcome,
    timeout_seconds: u32,
) -> (String, i32) {
    match outcome {
        SensorFunctionOutcome::Handled { .. } => {
            ("Handled: the function returned None.\n".into(), 0)
        }
        SensorFunctionOutcome::Skipped { reason, .. } => (format!("Skipped: {reason}\n"), 0),
        SensorFunctionOutcome::Requested { runs, .. } => {
            let mut text = format!(
                "The function asked for {} run{}; try starts none:\n",
                runs.len(),
                if runs.len() == 1 { "" } else { "s" }
            );
            for run in runs {
                let _ = writeln!(text, "  {}  (run key {})", run.job, run.run_key);
            }
            (text, 0)
        }
        SensorFunctionOutcome::Failed { error } => {
            (format!("{error}\n\nThe function failed.\n"), 1)
        }
        SensorFunctionOutcome::TimedOut => (
            format!("The function timed out after {timeout_seconds} seconds.\n"),
            1,
        ),
    }
}
