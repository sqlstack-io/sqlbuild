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
/// Finished ticks read back to find when a failing sensor started failing.
pub(crate) const FAILING_TICKS_READ: usize = 200;
/// Events `sqb sensor try` lists at most.
pub(crate) const MAX_TRIAL_EVENTS: usize = 100;
