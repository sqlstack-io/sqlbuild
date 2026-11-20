//! Public constants of compiled project.

pub const SKIP_ACTION: &str = "skip";

pub const NO_CHANGE_REASON: &str = "no_change";

pub const REPORT_ATTEMPTS: usize = 3;

/// The HTTP status a control server answers while its store is busy; reports retry it.
pub const STORE_BUSY_STATUS: u16 = 503;

/// Selector kinds that can name a job: `job:<name>` and a bare name.
pub const JOB_SELECTOR_KINDS: [&str; 2] = ["job", "name"];

/// Separates the parts of an intersection selector.
pub const SELECTOR_INTERSECTION: char = ',';

/// The key prefix of a SQL unit test's result, as the snapshot names it.
pub const SQL_TEST_KEY_PREFIX: &str = "sql_test:";

/// The sensor argument naming the job it starts, as the orchestration inventory labels it.
pub(crate) const SENSOR_RUN_ARGUMENT: &str = "run";

pub const CONTRACT_KEY: &str = "contract";

pub const INCREMENTAL_MODE_KEY: &str = "incremental_mode";

/// The build settings a job run's `--changed` reach reads, and the reach every job run uses.
pub const CHANGED_SETTING: &str = "changed";
pub const REACH_SETTING: &str = "reach";
/// A job run reaches its whole selection: `reach` only stops builds that are not a job's run.
pub const JOB_RUN_REACH: &str = "all";
/// What a job run with `changed: True` counts when `[changes] counts` is not set: every change.
pub const JOB_RUN_COUNTS: [&str; 2] = ["definition", "source_data"];
/// Plan metadata naming the jobs whose own audits a build lets fail.
pub const JOB_IGNORED_AUDIT_FAILURES_KEY: &str = "job_ignored_audit_failures";
/// Plan metadata naming the jobs whose own models a build rebuilds from scratch.
pub const JOB_FULL_REFRESH_KEY: &str = "job_full_refresh";
/// Plan metadata mapping each named job whose warehouse its nodes run on to that warehouse.
pub const JOB_WAREHOUSES_KEY: &str = "job_warehouses";
/// Plan metadata mapping each named job whose own nodes run at most so many at once to that limit.
pub const JOB_CONCURRENCY_KEY: &str = "job_concurrency";
