//! Acknowledgements of problem runs, kept as audit events.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::acknowledgements::constants::{
    ATTENTION_LISTING_SNOOZES, DEFAULT_ATTENTION_SNOOZE_SECONDS, MAX_ATTENTION_SNOOZE_DAYS,
    NO_TIME_LIMIT,
};
use crate::credentials::models::DefaultRole;
use crate::store::errors::ControlError;

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

/// Dismisses the problem of one stored event webhooks and sensors skipped because it can't be read; an event ID applies once.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SkippedEventDismissalRequest {
    pub event_id: Uuid,
    /// The skipped stored event's storage ID, as the problem lists it.
    pub storage_id: i64,
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
    /// Why, when it isn't a person's own choice, such as a withdrawal once the condition cleared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// For a withdrawal, the acknowledgement event it withdraws: it applies only while that is still the latest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub withdraws: Option<Uuid>,
}

/// A condition whose latest acknowledgement event marks it handled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AttentionAcknowledgement {
    /// The acknowledging audit event, which a withdrawal can name.
    pub event_id: Uuid,
    pub key: String,
    pub shape: serde_json::Value,
    /// The person's name, or the audit actor when the person no longer exists.
    pub acknowledged_by: String,
    pub acknowledged_at: DateTime<Utc>,
    /// When the acknowledgement lapses even if the condition never changes; none without a time limit.
    pub until: Option<DateTime<Utc>>,
}

/// A target's condition acknowledgements, and how long one lasts while its condition is unchanged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AttentionAcknowledgements {
    /// The server's `attention_snooze` in seconds; none when acknowledgements have no time limit.
    pub snooze_seconds: Option<i64>,
    pub acknowledgements: Vec<AttentionAcknowledgement>,
}

/// How long an acknowledged condition stays hidden while it doesn't change; `None` for no time limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttentionSnooze(Option<Duration>);

impl AttentionSnooze {
    /// A duration in the project's format, such as `24h` or `1d12h`, or `0` for no time limit.
    pub fn parse(text: &str) -> Result<Self, ControlError> {
        let invalid = || {
            ControlError::Invalid(format!(
                "attention_snooze must be a duration such as \"24h\" or \"1d12h\", or \"0\" for no time limit, not {text:?}"
            ))
        };
        if text.trim() == NO_TIME_LIMIT {
            return Ok(Self(None));
        }
        let digits = sqlbuild_cursor::algebra::main::parse_duration::parse_duration(text)
            .ok_or_else(invalid)?;
        let amount = |digits: &str| digits.parse::<i64>().map_err(|_| invalid());
        if amount(&digits.years)? != 0 || amount(&digits.months)? != 0 {
            return Err(ControlError::Invalid(format!(
                "attention_snooze can't use years or months, which vary in length: {text:?}"
            )));
        }
        let seconds = [
            (digits.days, 86_400),
            (digits.hours, 3_600),
            (digits.minutes, 60),
            (digits.seconds, 1),
        ]
        .into_iter()
        .try_fold(0_i64, |total, (digits, unit)| {
            amount(&digits)?
                .checked_mul(unit)
                .and_then(|part| total.checked_add(part))
                .ok_or_else(invalid)
        })?;
        if seconds > MAX_ATTENTION_SNOOZE_DAYS * 86_400 {
            return Err(ControlError::Invalid(format!(
                "attention_snooze can be at most {MAX_ATTENTION_SNOOZE_DAYS} days, or \"0\" for no time limit: {text:?}"
            )));
        }
        Duration::try_seconds(seconds)
            .map(|snooze| Self(Some(snooze)))
            .ok_or_else(invalid)
    }

    /// The snooze in seconds; none without a time limit.
    pub fn seconds(self) -> Option<i64> {
        self.0.map(|snooze| snooze.num_seconds())
    }

    /// When an acknowledgement made `at` lapses unchanged; none without a time limit.
    pub(crate) fn until(self, at: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.0.map(|snooze| at + snooze)
    }

    /// How far back a listing reads; without a time limit, every acknowledgement the audit log keeps.
    pub(crate) fn listing_window(self) -> Option<Duration> {
        self.0.map(|snooze| snooze * ATTENTION_LISTING_SNOOZES)
    }
}

impl Default for AttentionSnooze {
    fn default() -> Self {
        Self(Some(Duration::seconds(DEFAULT_ATTENTION_SNOOZE_SECONDS)))
    }
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
    /// None to read every event the audit log keeps.
    pub(crate) window: Option<Duration>,
}

/// A subject's standing acknowledgement: who made it, when, and the event's scope.
#[derive(Debug)]
pub(crate) struct LatestAcknowledgement {
    pub(crate) event_id: Uuid,
    pub(crate) subject: String,
    pub(crate) acknowledged_by: String,
    pub(crate) acknowledged_at: DateTime<Utc>,
    pub(crate) scope: Value,
}

/// A condition's acknowledgement as stored before a request: the request's own retry, or the standing state.
#[derive(Debug)]
pub(crate) enum PriorAttention {
    Retried,
    Standing(StandingAttention),
}

/// A condition's latest acknowledgement event, if any: whether it acknowledges, with which shape, when.
#[derive(Debug)]
pub(crate) struct StandingAttention {
    pub(crate) event_id: Option<Uuid>,
    pub(crate) acknowledged: bool,
    pub(crate) shape: Value,
    pub(crate) at: Option<DateTime<Utc>>,
}
