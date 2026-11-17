//! Result status and outcome values the aggregation reads.

pub(crate) const SUCCESS: &str = "success";
pub(crate) const FAILED: &str = "failed";
pub(crate) const SKIPPED: &str = "skipped";
pub(crate) const TEST_PASS: &str = "pass";
pub(crate) const AUDIT_WARN: &str = "warn";
pub(crate) const AUDIT_ERROR: &str = "error";
pub(crate) const AUDIT_INSUFFICIENT: &str = "insufficient";
pub(crate) const AUDIT_SKIPPED: &str = "skipped";
/// The outcome a fact names an error-severity audit failure by when the build ignored it.
pub const IGNORED_AUDIT_FAILURE: &str = "ignored";
pub(crate) const SOURCE_FRESHNESS_BLOCKED_CODE: &str = "B005";
pub(crate) const TEST_TIMEOUT: &str = "timeout";
pub(crate) const ITEM_TIMEOUT_CODE: &str = "C417";
