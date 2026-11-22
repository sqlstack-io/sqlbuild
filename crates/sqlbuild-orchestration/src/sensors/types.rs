//! Compound sensor return values.

use crate::store::models::ControlTransaction;

/// Selected source definitions by node key, and the key the next page starts after.
pub(crate) type SourcePage = (
    ControlTransaction,
    (Vec<(String, serde_json::Value)>, Option<String>),
);

/// A sensor's newest requested evaluation and its terminal phase (`None` while in flight), or why it no longer decodes.
pub(crate) type LastAttempt =
    Result<(crate::sensors::models::SensorEvaluationFact, Option<String>), String>;

/// An end of a failing streak: when its tick started, then why it failed, its event and that event's summary.
pub(crate) type StreakEnd = (
    chrono::DateTime<chrono::Utc>,
    (Option<String>, Option<String>, Option<String>),
);
