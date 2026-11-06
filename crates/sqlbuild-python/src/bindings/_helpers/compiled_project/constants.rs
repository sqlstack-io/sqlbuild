//! Frozen execution protocol values and bounded acknowledgement retries.

pub(crate) const SKIP_ACTION: &str = "skip";
pub(crate) const NO_CHANGE_REASON: &str = "no_change";
pub(crate) const REPORT_ATTEMPTS: usize = 3;
