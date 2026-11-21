//! Bounds on what one sensor tick reads, requests and waits for.

pub(crate) const EXECUTION_NODE_BATCH_SIZE: usize = 500;
pub(crate) const SENSOR_RESULT_RECHECK_SECONDS: i64 = 1;
/// Runs, nodes, checks or sources one tick reads while it looks for an event.
pub(crate) const EVENT_PAGE_SIZE: usize = 100;
/// How long past its timeout a sensor's location work may wait to be claimed and answered.
pub(crate) const SENSOR_WORK_GRACE_SECONDS: i64 = 300;
pub(crate) const SOURCE_NODE_KIND: &str = "source";
pub(crate) const TIMESTAMP_VALUE_KIND: &str = "timestamp";
pub const MAX_SENSOR_RUN_REQUESTS: usize = 100;
/// Pages a person's look at recent events reads back at most.
pub(crate) const RECENT_EVENT_PAGES: usize = 10;
/// Events `sqb sensor try` lists at most.
pub(crate) const MAX_TRIAL_EVENTS: usize = 100;
/// The runtime request key of `sqb sensor try`: `{"try_sensor": SensorTrial}`.
pub const TRY_SENSOR_REQUEST: &str = "try_sensor";
/// A tick the location hasn't answered yet, as people see it.
pub(crate) const TICK_RUNNING: &str = "running";
/// A tick whose function returned `None`.
pub(crate) const TICK_HANDLED: &str = "handled";
/// A tick whose function returned a `SkipReason`.
pub(crate) const TICK_SKIPPED: &str = "skipped";
/// A tick whose function asked for runs.
pub(crate) const TICK_STARTED: &str = "started";
/// The rows `node_event` reads: started nodes and checks of job runs, not of group runs.
pub(crate) const NODE_EVENT_ROWS: &str = " AND n.started_at IS NOT NULL AND r.kind<>'group'";
/// The rows `check_event` reads: checks, audits and SQL tests of any run, including those a run selected, which it records as nodes.
pub(crate) const CHECK_EVENT_ROWS: &str = " AND (n.kind='check' OR n.node_key LIKE 'audit:%' OR n.node_key LIKE 'sql_test:%' OR n.node_key LIKE 'check:%')";
/// The key prefixes of checks, audits and SQL tests, whether a run recorded them as checks or as selected nodes.
pub(crate) const CHECK_KEY_PREFIXES: [&str; 3] = ["audit:", "sql_test:", "check:"];
/// The `sqb_run_nodes.kind` of a check a run didn't select.
pub(crate) const CHECK_ROW_KIND: &str = "check";
/// The tick phases a failing streak counts: failed, timed out, or abandoned because the location didn't answer.
pub(crate) const FAILING_TICK_PHASES: &str = "(phase IN ('failed','timed_out') OR (phase='abandoned' AND fact_json -> 'outcome' ->> 'reason'='unanswered'))";
/// The longest one call of a sensor's function may take: a running sensor holds its location's other sensors, plans and sync until it returns.
pub const MAX_SENSOR_TIMEOUT_SECONDS: u32 = 600;
