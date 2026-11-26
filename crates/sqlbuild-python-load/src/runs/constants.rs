//! What a load's threads need.

/// The stack every thread of a load runs on: a debug build's load goes deeper than Tokio's 2 MiB default.
pub(crate) const LOAD_STACK_BYTES: usize = 8 * 1024 * 1024;
