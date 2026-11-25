//! The health report anyone may read: each component's state, since when, and a short reason.

use chrono::{DateTime, TimeDelta, Utc};

use crate::health::constants::{
    CONNECTING, CONTROL_DATABASE_NAME, HEALTH_PROBE_INTERVAL, SCHEDULER_NAME, SCHEDULER_STARTING,
    SERVER_NAME,
};
use crate::health::models::{
    ComponentHealth, ComponentState, HealthComponent, HealthReport, HealthStatus, SeenLocation,
    ServerHealth, ServerSummary,
};
use crate::platform::constants::LOCATION_SILENCE_SECONDS;

/// The server's health at `now`, read from memory only.
#[must_use]
pub fn health_report(health: &ServerHealth, now: DateTime<Utc>) -> HealthReport {
    let state = health.state();
    let mut components: Vec<ComponentHealth> = vec![
        ComponentHealth {
            component: HealthComponent::Server,
            name: SERVER_NAME.to_owned(),
            location_id: None,
            status: HealthStatus::Healthy,
            since: Some(health.started_at),
            reason: None,
            case: None,
            last_success: Some(now),
            last_failure: None,
        },
        component(
            (HealthComponent::ControlDatabase, CONTROL_DATABASE_NAME),
            &state.control_database,
            (HealthStatus::Unavailable, CONNECTING, health.started_at),
        ),
    ];
    components.extend(state.scheduler.as_ref().map(|scheduler| {
        component(
            (HealthComponent::Scheduler, SCHEDULER_NAME),
            scheduler,
            (
                HealthStatus::Degraded,
                SCHEDULER_STARTING,
                health.started_at,
            ),
        )
    }));
    components.extend(
        state
            .locations
            .iter()
            .map(|(location_id, location)| ComponentHealth {
                location_id: Some(*location_id),
                ..location_health(location, now)
            }),
    );
    let status: HealthStatus = components
        .iter()
        .map(|component| component.status)
        .max()
        .unwrap_or(HealthStatus::Healthy);
    HealthReport {
        status,
        checked_at: now,
        retry_seconds: HEALTH_PROBE_INTERVAL.as_secs(),
        server: ServerSummary {
            version: health.version.clone(),
            started_at: health.started_at,
            uptime_seconds: (now - health.started_at).num_seconds().max(0),
        },
        components,
    }
}

/// A component from its state: failing is `failing`'s status; never succeeded nor failed is `starting`'s.
fn component(
    (kind, name): (HealthComponent, &str),
    state: &ComponentState,
    (failing, starting, started_at): (HealthStatus, &str, DateTime<Utc>),
) -> ComponentHealth {
    let failure = state.last_failure.as_ref();
    let base = ComponentHealth {
        component: kind,
        name: name.to_owned(),
        location_id: None,
        status: HealthStatus::Healthy,
        since: state.last_success,
        reason: None,
        case: None,
        last_success: state.last_success,
        last_failure: failure.map(|failure| failure.at),
    };
    match failure {
        Some(failure) if state.failing() => ComponentHealth {
            status: failing,
            since: Some(failure.since),
            reason: Some(failure.reason.clone()),
            case: failure.case,
            ..base
        },
        None if state.last_success.is_none() => ComponentHealth {
            status: failing,
            since: Some(started_at),
            reason: Some(starting.to_owned()),
            ..base
        },
        _ => base,
    }
}

/// A location is degraded once it hasn't reported for the silence a location is allowed.
fn location_health(location: &SeenLocation, now: DateTime<Utc>) -> ComponentHealth {
    let silent = location
        .last_report
        .is_some_and(|last| now - last > TimeDelta::seconds(LOCATION_SILENCE_SECONDS));
    ComponentHealth {
        component: HealthComponent::Location,
        name: location.name.clone(),
        location_id: None,
        status: if silent {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        },
        since: location.last_report,
        reason: silent.then(|| {
            format!(
                "Hasn't reported since {}",
                location
                    .last_report
                    .unwrap_or(now)
                    .format("%Y-%m-%d %H:%M UTC")
            )
        }),
        case: None,
        last_success: location.last_report,
        last_failure: None,
    }
}
