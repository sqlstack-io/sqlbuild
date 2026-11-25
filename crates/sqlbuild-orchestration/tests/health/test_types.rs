use crate::health::models::{ControlCauseCase, HealthStatus};
use crate::store::errors::ControlError;

pub(crate) struct CauseResponseTestCase {
    pub(crate) description: &'static str,
    pub(crate) error: fn() -> ControlError,
    pub(crate) expected_status: u16,
    pub(crate) expected_message: &'static str,
    pub(crate) expected_case: ControlCauseCase,
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
