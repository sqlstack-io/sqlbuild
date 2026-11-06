//! Delivery bounds shared by independent native event consumers.

/// Only server operators configure this destination; never inherit it in a location or worker.
pub const SECURITY_AUDIT_WEBHOOK_VARIABLE: &str = "SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL";

pub(crate) const REQUEST_TIMEOUT_SECONDS: u64 = 10;
pub(crate) const POLL_SECONDS: u64 = 5;
