//! How long captured build output waits before its next heartbeat line.

use std::time::Duration;

use crate::build_progress::constants::HEARTBEAT_INTERVALS;

/// The wait before heartbeat `beats + 1`: a node must first run 30 s, then 1 min, 2 min, and every 5 min after.
#[must_use]
pub fn heartbeat_interval(beats: usize) -> Duration {
    let last: usize = HEARTBEAT_INTERVALS.len() - 1;
    Duration::from_secs(HEARTBEAT_INTERVALS[beats.min(last)])
}
