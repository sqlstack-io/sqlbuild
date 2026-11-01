//! Bounds shared by the platform event reader and webhook dispatcher.

/// Operator-owned delivery endpoint; never inherited by project workers or printed in logs.
pub const PLATFORM_WEBHOOK_VARIABLE: &str = "SQLBUILD_PLATFORM_WEBHOOK_URL";

pub(crate) const SINK_PAGE_SIZE: i64 = 100;
pub(crate) const SINK_REQUEST_TIMEOUT_SECONDS: u64 = 10;
pub(crate) const SINK_POLL_SECONDS: u64 = 5;
