use std::borrow::Cow;

use crate::health::models::{ControlCauseCase, HealthStatus};
use crate::store::errors::ControlError;

pub(crate) struct CauseResponseTestCase {
    pub(crate) description: &'static str,
    pub(crate) error: fn() -> ControlError,
    pub(crate) expected_status: u16,
    pub(crate) expected_message: &'static str,
    pub(crate) expected_case: Option<ControlCauseCase>,
    /// Whether the server's health turns unavailable with it.
    pub(crate) expected_unavailable: bool,
}

pub(crate) struct ReportTestCase {
    pub(crate) description: &'static str,
    pub(crate) events: fn(&crate::health::models::ServerHealth, chrono::DateTime<chrono::Utc>),
    pub(crate) expected_status: HealthStatus,
    /// Each component's name, status and reason.
    pub(crate) expected_components: Vec<(&'static str, HealthStatus, Option<&'static str>)>,
}

/// A database error with a SQLSTATE, as PostgreSQL returns one.
#[derive(Debug)]
pub(crate) struct CodedError {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
}

impl std::fmt::Display for CodedError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for CodedError {}

impl sqlx_core::error::DatabaseError for CodedError {
    fn message(&self) -> &str {
        self.message
    }

    fn code(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed(self.code))
    }

    fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
        self
    }

    fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
        self
    }

    fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
        self
    }

    fn kind(&self) -> sqlx_core::error::ErrorKind {
        sqlx_core::error::ErrorKind::Other
    }
}

pub(crate) struct PoolTimeoutTestCase {
    pub(crate) description: &'static str,
    pub(crate) pool_full: bool,
    pub(crate) expected_cause: Option<(ControlCauseCase, &'static str, bool)>,
}

pub(crate) struct ForeignErrorTestCase {
    pub(crate) description: &'static str,
    pub(crate) error: fn() -> ControlError,
    pub(crate) expected_cause: Option<ControlCauseCase>,
}

pub(crate) struct FailingStreakTestCase {
    pub(crate) description: &'static str,
    /// Minutes after the first failure at which the database fails again.
    pub(crate) failures_after: Vec<i64>,
    pub(crate) expected_since_minutes: i64,
}

pub(crate) struct ForwardingTestCase {
    pub(crate) description: &'static str,
    /// Each delivery attempt: seconds after the start, and whether it delivered.
    pub(crate) outcomes: Vec<(i64, bool)>,
    /// The server's status, the forwarding row's since in seconds after the start, and its reason.
    pub(crate) expected_state: (HealthStatus, Option<i64>, Option<&'static str>),
}
