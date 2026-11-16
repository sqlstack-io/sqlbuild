//! Public constants of compiled project.

pub const SKIP_ACTION: &str = "skip";

pub const NO_CHANGE_REASON: &str = "no_change";

pub const REPORT_ATTEMPTS: usize = 3;

/// The HTTP status a control server answers while its store is busy; reports retry it.
pub const STORE_BUSY_STATUS: u16 = 503;

/// Selector kinds that can name a job: `job:<name>` and a bare name.
pub(crate) const JOB_SELECTOR_KINDS: [&str; 2] = ["job", "name"];

/// Separates the parts of an intersection selector.
pub(crate) const SELECTOR_INTERSECTION: char = ',';

/// The key prefix of a SQL unit test's result, as the snapshot names it.
pub const SQL_TEST_KEY_PREFIX: &str = "sql_test:";

/// The sensor argument naming the job it starts, as the orchestration inventory labels it.
pub(crate) const SENSOR_RUN_ARGUMENT: &str = "run";

pub const CONTRACT_KEY: &str = "contract";

pub const INCREMENTAL_MODE_KEY: &str = "incremental_mode";
