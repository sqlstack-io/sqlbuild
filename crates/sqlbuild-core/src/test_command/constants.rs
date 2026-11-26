//! Test output labels and widths.

pub(crate) const DEFAULT_ERROR: &str = "test failed";
pub(crate) const UNKNOWN_GROUP: &str = "(unknown)";
pub(crate) const ERROR_SEVERITY: &str = "error";
pub(crate) const ASSERTION_PREFIX: &str = "assertion ";
pub(crate) const PASS_OUTCOME: &str = "pass";
pub(crate) const FAIL_OUTCOME: &str = "fail";
/// The error a test reports when an expectation failed; its expectations' rows say more, so it is left out.
pub(crate) const EXPECTATIONS_FAILED_CODE: &str = "T003";
pub(crate) const TIMED_OUT_REASON: &str = "timed out";
pub(crate) const ASSERTION_FAILED_REASON: &str = "assertion failed";
