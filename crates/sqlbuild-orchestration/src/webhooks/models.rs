//! A closed event source determines its independent durable acknowledgement namespace.

use crate::credentials::models::SecurityAuditEvent;
use crate::platform::models::PlatformEventEnvelope;

#[derive(Clone, Copy, Debug)]
pub enum WebhookSource {
    Platform,
    SecurityAudit,
}

impl WebhookSource {
    pub(crate) const fn deliveries(self) -> &'static str {
        match self {
            Self::Platform => "sqb_platform_sink_deliveries",
            Self::SecurityAudit => "sqb_security_audit_deliveries",
        }
    }

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Platform => "Platform",
            Self::SecurityAudit => "Security audit",
        }
    }
}

/// Neither errors nor Debug may expose an operator endpoint containing credentials.
#[derive(Clone)]
pub struct Webhook {
    pub(crate) client: reqwest::Client,
    pub(crate) url: reqwest::Url,
    pub(crate) sink_id: String,
    pub(crate) source: WebhookSource,
}

impl std::fmt::Debug for Webhook {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Webhook")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

/// Preserve the existing source envelope, not a delivery-specific copy of its fields.
#[derive(Debug, serde::Serialize)]
#[serde(untagged)]
pub(crate) enum WebhookEvent {
    Platform(PlatformEventEnvelope),
    SecurityAudit(Box<SecurityAuditEvent>),
}

impl WebhookEvent {
    pub(crate) fn identity(&self) -> (i64, uuid::Uuid) {
        match self {
            Self::Platform(event) => (event.storage_id, event.event_id),
            Self::SecurityAudit(event) => (event.storage_id, event.event_id),
        }
    }
}
