//! Acknowledgements of problem runs, kept as audit events.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::credentials::models::DefaultRole;

/// Marks a run as handled for everyone who can see it, or withdraws the mark; retried with the same event ID it applies once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcknowledgementRequest {
    pub event_id: Uuid,
    pub run_id: Uuid,
    pub acknowledged: bool,
}

/// A run whose latest acknowledgement event marks it handled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct RunAcknowledgement {
    pub run_id: Uuid,
    /// The person's name, or the audit actor when the person no longer exists.
    pub acknowledged_by: String,
    pub acknowledged_at: DateTime<Utc>,
}

/// Marks an ongoing condition handled as it looks now, or withdraws that; an event ID applies once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AttentionAcknowledgementRequest {
    pub event_id: Uuid,
    /// The condition's kind and subject, stable while it lasts, such as `stale_sources`.
    pub key: String,
    /// What the condition looks like, as a JSON object of at most 8 KiB that later states are compared with.
    pub shape: serde_json::Value,
    pub acknowledged: bool,
}

/// A condition whose latest acknowledgement event marks it handled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AttentionAcknowledgement {
    pub key: String,
    pub shape: serde_json::Value,
    /// The person's name, or the audit actor when the person no longer exists.
    pub acknowledged_by: String,
    pub acknowledged_at: DateTime<Utc>,
    /// When the acknowledgement lapses even if the condition never changes.
    pub until: DateTime<Utc>,
}

/// The audit actions of one kind of acknowledgement, and the scope field naming its subject.
#[derive(Debug)]
pub(crate) struct AcknowledgementEvents {
    pub(crate) acknowledged: &'static str,
    pub(crate) withdrawn: &'static str,
    pub(crate) subject: &'static str,
}

/// Which target's acknowledgements of one kind to list, over how long.
#[derive(Debug)]
pub(crate) struct AcknowledgementListing<'a> {
    pub(crate) location_id: Uuid,
    pub(crate) target: &'a str,
    pub(crate) default_role: DefaultRole,
    pub(crate) events: &'a AcknowledgementEvents,
    pub(crate) window: Duration,
}

/// A subject's standing acknowledgement: who made it, when, and the event's scope.
#[derive(Debug)]
pub(crate) struct LatestAcknowledgement {
    pub(crate) subject: String,
    pub(crate) acknowledged_by: String,
    pub(crate) acknowledged_at: DateTime<Utc>,
    pub(crate) scope: Value,
}

/// A condition's acknowledgement as stored before a request: the request's own retry, or the standing state.
#[derive(Debug)]
pub(crate) enum PriorAttention {
    Retried,
    Standing {
        acknowledged: bool,
        shape: Value,
        at: Option<DateTime<Utc>>,
    },
}
