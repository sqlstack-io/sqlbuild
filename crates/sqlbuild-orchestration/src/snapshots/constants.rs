//! Snapshot retention keeps recent publications per location and target, collected in small batches.

pub(crate) const DEFAULT_SNAPSHOT_RETENTION_COUNT: u16 = 10;
pub(crate) const SNAPSHOT_RETENTION_BATCH: usize = 10;
pub(crate) const CONTENT_HASH_CHUNK: usize = 500;
