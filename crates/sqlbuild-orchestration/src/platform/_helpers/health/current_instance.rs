//! The newest location instance and whether its own first load has finished, from one target's observations.

use chrono::{DateTime, TimeDelta, Utc};
use uuid::Uuid;

use crate::platform::constants::{LOCATION_RETENTION_DAYS, RECENT_HEALTH_LIMIT};
use crate::platform::models::{LocationHealth, LocationHealthObservation};

/// Where the newest live instance of a target stands.
pub(crate) enum CurrentInstance {
    /// Its first load has not finished: judge nothing from earlier instances.
    Starting,
    /// Its first load finished; its own reports from that load on are its health.
    Loaded(Uuid),
    /// The observations can't tell: an older location, or one whose start has left the history.
    Unknown,
}

/// The newest live instance, by its time-ordered session ID, and whether its first load finished.
pub(crate) fn current_instance(
    (own, read): (&[&LocationHealthObservation], usize),
    (silence, now): (TimeDelta, DateTime<Utc>),
) -> CurrentInstance {
    let Some(session) = own
        .iter()
        .filter(|observation| is_check_in(&observation.report.health))
        .filter(|observation| now - observation.received_at <= silence)
        .map(|observation| observation.report.session_id)
        .max()
    else {
        return CurrentInstance::Unknown;
    };
    if own.iter().any(|observation| {
        observation.report.session_id == session && is_load_outcome(&observation.report.health)
    }) {
        return CurrentInstance::Loaded(session);
    }
    let Some(started) = session_started(session) else {
        return CurrentInstance::Unknown;
    };
    if history_covers((own, read), (started, now)) {
        CurrentInstance::Starting
    } else {
        CurrentInstance::Unknown
    }
}

/// A check-in: the instance's liveness row, which holds its newest heartbeat.
fn is_check_in(health: &LocationHealth) -> bool {
    matches!(health, LocationHealth::Heartbeat {})
}

/// A report that ends a load: a failed compile, or the connection check that follows a publication.
fn is_load_outcome(health: &LocationHealth) -> bool {
    matches!(
        health,
        LocationHealth::CompileFailed { .. } | LocationHealth::Connection { .. }
    )
}

/// When the process that made a time-ordered (v7) session ID started; none for other IDs.
fn session_started(session: Uuid) -> Option<DateTime<Utc>> {
    let (seconds, nanos) = session.get_timestamp()?.to_unix();
    DateTime::from_timestamp(i64::try_from(seconds).ok()?, nanos)
}

/// Whether the history reaches back to `started`: none of the `read` observations was cut off, or an older one is kept.
fn history_covers(
    (own, read): (&[&LocationHealthObservation], usize),
    (started, now): (DateTime<Utc>, DateTime<Utc>),
) -> bool {
    if now - started > TimeDelta::days(LOCATION_RETENTION_DAYS) {
        return false;
    }
    read < RECENT_HEALTH_LIMIT
        || own.iter().any(|observation| {
            !is_check_in(&observation.report.health) && observation.received_at <= started
        })
}
