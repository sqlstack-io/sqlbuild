//! Frozen execution protocol values and bounded acknowledgement retries.

pub(crate) const SKIP_ACTION: &str = "skip";
pub(crate) const NO_CHANGE_REASON: &str = "no_change";
pub(crate) const REPORT_ATTEMPTS: usize = 3;
/// The HTTP status a control server answers while its store is busy; reports retry it.
pub(crate) const STORE_BUSY_STATUS: u16 = 503;
