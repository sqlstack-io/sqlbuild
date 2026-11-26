//! What `sqb location health` exits with for each state, and what the location's interpreters' sinks delivered.

use std::time::Duration;

use sqlbuild_orchestration::platform::constants::SINK_DELIVERIES_REQUEST;
use sqlbuild_orchestration::platform::models::{LocationHealth, LocationState, SinkDeliveryTotals};
use sqlbuild_orchestration::runs::models::SinkDelivery;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::time::Instant;

use uuid::Uuid;

use crate::argv::models::Stream;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::execution::{
    exchange_reply, report_location_health, stop_location_runtime,
};
use crate::native_commands::constants::{SINK_CHECK_SECONDS, SINK_CHECK_TIMEOUT_SECONDS};
use crate::native_commands::models::{LocationCompiler, LocationServe, SinkTally};

use crate::native_commands::constants::STARTING_EXIT_CODE;

/// 0 only when healthy; 1 when anything failed or is down; 3 while the newest instance's first load runs, or 0 then when `allow_starting`, as a container health check counts a starting location alive.
pub(crate) fn health_exit_code(state: LocationState, allow_starting: bool) -> i32 {
    match state {
        LocationState::Healthy => 0,
        LocationState::Starting if allow_starting => 0,
        LocationState::Starting => STARTING_EXIT_CODE,
        LocationState::Broken
        | LocationState::FallingBack
        | LocationState::MissingSecrets
        | LocationState::ChangePlanFailed
        | LocationState::DiscoveryFailed
        | LocationState::Degraded => 1,
    }
}

/// Each sink's change between two totals, keeping only sinks that took events or lost some.
pub(crate) fn sink_interval(
    previous: &[SinkDelivery],
    current: &[SinkDelivery],
) -> Vec<SinkDelivery> {
    let mut changed: Vec<SinkDelivery> = Vec::new();
    for sink in current {
        let before: Option<&SinkDelivery> =
            previous.iter().find(|earlier| earlier.name == sink.name);
        let since =
            |count: fn(&SinkDelivery) -> u64| count(sink).saturating_sub(before.map_or(0, count));
        let interval = SinkDelivery {
            name: sink.name.clone(),
            accepted: since(|sink| sink.accepted),
            delivered: since(|sink| sink.delivered),
            dropped: since(|sink| sink.dropped),
            failed: since(|sink| sink.failed),
        };
        if interval.accepted > 0 || interval.dropped > 0 || interval.failed > 0 {
            changed.push(interval);
        }
    }
    changed
}

/// When `compiler` is next due a sink check.
pub(crate) fn sink_due(compiler: &LocationCompiler) -> Instant {
    compiler
        .sinks
        .checked_at
        .map_or_else(Instant::now, |checked| {
            checked + Duration::from_secs(SINK_CHECK_SECONDS)
        })
}

/// Once due, ask `compiler` what its sinks delivered: it back, with the interval's report if they took or lost events.
pub(crate) async fn sink_check(
    mut compiler: LocationCompiler,
) -> (
    LocationCompiler,
    Result<Option<LocationHealth>, ControlError>,
) {
    if sink_due(&compiler) > Instant::now() {
        return (compiler, Ok(None));
    }
    let request = serde_json::json!({ SINK_DELIVERIES_REQUEST: {} });
    let answered = tokio::time::timeout(
        Duration::from_secs(SINK_CHECK_TIMEOUT_SECONDS),
        exchange_reply::<SinkDeliveryTotals>(&mut compiler, &request, "sink delivery totals"),
    )
    .await;
    let totals: SinkDeliveryTotals = match answered {
        Ok(Ok(totals)) => totals,
        Ok(Err(error)) => return (compiler, Err(error)),
        Err(_) => {
            let silent = "the interpreter didn't say what its sinks delivered";
            return (compiler, Err(ControlError::Invalid(silent.into())));
        }
    };
    let sinks: Vec<SinkDelivery> = sink_interval(&compiler.sinks.totals, &totals.sinks);
    compiler.sinks = SinkTally {
        totals: totals.sinks,
        checked_at: Some(Instant::now()),
    };
    (
        compiler,
        Ok((!sinks.is_empty()).then_some(LocationHealth::SinkDeliveries { sinks })),
    )
}

/// Report what an idle `compiler`'s sinks delivered once due; an interpreter that can't say is stopped.
pub(crate) async fn report_compiler_sinks(
    compiler: Option<LocationCompiler>,
    (options, (bearer, session_id)): (&LocationServe, (&str, Uuid)),
) -> Result<Option<LocationCompiler>, ControlError> {
    let Some(active) = compiler else {
        return Ok(None);
    };
    let (active, checked) = sink_check(active).await;
    match checked {
        Ok(None) => Ok(Some(active)),
        Ok(Some(health)) => {
            report_location_health(options, bearer, session_id, health).await;
            Ok(Some(active))
        }
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!(
                    "Checking event sink deliveries failed: {error}; the location restarts its interpreter\n"
                ),
            )]));
            stop_location_runtime(Some(active)).await?;
            Ok(None)
        }
    }
}
