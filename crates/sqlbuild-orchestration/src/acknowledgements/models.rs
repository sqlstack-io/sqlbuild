//! Acknowledgements of problem runs, kept as audit events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// Marks an ongoing attention condition handled, as it looks now, or withdraws the mark; retried with
/// the same event ID it applies once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AttentionAcknowledgementRequest {
    pub event_id: Uuid,
    /// The condition: its kind and subject, stable while it lasts, such as `stale_sources` or
    /// `event:tick_overdue:<schedule>`.
    pub key: String,
    /// What the condition looked like (counts, members, error codes), which later states are compared
    /// with; a JSON object of at most 8 KiB. Never message text, which can hold relative times.
    #[schema(value_type = Object)]
    pub shape: serde_json::Value,
    pub acknowledged: bool,
}

/// A condition whose latest acknowledgement event marks it handled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AttentionAcknowledgement {
    pub key: String,
    #[schema(value_type = Object)]
    pub shape: serde_json::Value,
    /// The person's name, or the audit actor when the person no longer exists.
    pub acknowledged_by: String,
    pub acknowledged_at: DateTime<Utc>,
    /// When the acknowledgement lapses even if the condition never changes.
    pub until: DateTime<Utc>,
}
