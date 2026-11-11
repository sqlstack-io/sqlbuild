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
