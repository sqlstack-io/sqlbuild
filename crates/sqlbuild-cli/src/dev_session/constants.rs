//! The foreground warehouse connection, the location child's pipes and the run feed of `sqb dev`.

use std::time::Duration;

/// The statuses of SQLBuild's own progress rows, which the framed block leaves out.
pub(crate) const LIFECYCLE_STATUSES: [&str; 3] = ["START", "OK", "FAIL"];
/// The `sqb debug` row that says whether the warehouse answered.
pub(crate) const CONNECTION_TEST_LABEL: &str = "connection test";
/// The warehouse state when `sqb debug` printed no report.
pub(crate) const NO_REPORT: &str = "sqb debug did not report a connection; see l for its log";
/// The `sqb debug` configuration row that names the adapter.
pub(crate) const ADAPTER_LABEL: &str = "adapter";
/// A `sqb debug` row that passed.
pub(crate) const OK_STATUS: &str = "OK";
/// A `sqb debug` row that failed.
pub(crate) const ERROR_STATUS: &str = "ERROR";
/// Report sections searched for the first error when the connection test did not pass.
pub(crate) const REPORT_ERROR_SECTIONS: [&str; 2] = ["configuration", "providers"];
/// How often finished and newly started runs are read for event lines.
pub(crate) const RUN_POLL: Duration = Duration::from_secs(2);
/// The newest runs read per poll.
pub(crate) const RUN_PAGE: u32 = 20;
/// The warehouse row's detail while the foreground connection runs.
pub(crate) const CONNECTING: &str = "connecting";
/// How long the foreground `sqb debug`'s pipes are drained after it exits.
pub(crate) const DEBUG_DRAIN: Duration = Duration::from_secs(2);
