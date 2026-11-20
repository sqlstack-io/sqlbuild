//! Bound query parameters independently of the size of an authored selection.

pub(crate) const EXECUTION_NODE_BATCH_SIZE: usize = 500;
pub(crate) const CUSTOM_SENSOR_RECHECK_SECONDS: i64 = 1;
pub(crate) const SOURCE_NODE_KIND: &str = "source";
pub(crate) const TIMESTAMP_VALUE_KIND: &str = "timestamp";
pub const MAX_SENSOR_RUN_REQUESTS: usize = 100;
/// The health problem a sensor shows when its last evaluation, recorded by an earlier release, no longer decodes and it is evaluated as if for the first time.
pub(crate) const UNREADABLE_SENSOR_ATTEMPT: &str = "this sensor's last check was recorded by an earlier SQLBuild release and can't be read; it was checked again as if for the first time";
