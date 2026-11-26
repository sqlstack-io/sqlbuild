//! Test output labels and widths.

pub(crate) const DEFAULT_ERROR: &str = "test failed";
pub(crate) const UNKNOWN_GROUP: &str = "(unknown)";
pub(crate) const ERROR_SEVERITY: &str = "error";
pub(crate) const ASSERTION_PREFIX: &str = "assertion ";
pub(crate) const ASSERTION_LABEL: &str = "assertion";
pub(crate) const EXPECTED_LABEL: &str = "expected";
pub(crate) const FAIL_OUTCOME: &str = "fail";
/// The errors a test (`T003`) or scenario (`X506`, `X508`) reports when an expectation failed; the expectations' rows say more, so they are left out.
pub(crate) const EXPECTATIONS_FAILED_CODES: [&str; 3] = ["T003", "X506", "X508"];
pub(crate) const TIMED_OUT_REASON: &str = "timed out";
pub(crate) const ASSERTION_FAILED_REASON: &str = "assertion failed";
