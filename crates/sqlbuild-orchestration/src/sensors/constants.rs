//! Bound query parameters independently of the size of an authored selection.

pub(crate) const EXECUTION_NODE_BATCH_SIZE: usize = 500;
pub(crate) const CUSTOM_SENSOR_RECHECK_SECONDS: i64 = 1;
pub const MAX_SENSOR_RUN_REQUESTS: usize = 100;
