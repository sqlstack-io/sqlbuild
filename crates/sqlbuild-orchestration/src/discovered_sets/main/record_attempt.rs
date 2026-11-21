//! A set after one attempt: a success replaces its result, a failure keeps the last good one.

use chrono::{DateTime, Utc};

use crate::discovered_sets::_helpers::digest::discovered_digest;
use crate::discovered_sets::_helpers::problems::discovered_problem;
use crate::discovered_sets::models::{DiscoveredResult, DiscoveredSet, DiscoveredSetStatus};

/// `previous` after an attempt at `now` that returned `attempt`; a result that is not a valid set fails like an error.
pub fn record_attempt(
    previous: Option<&DiscoveredSet>,
    attempt: Result<DiscoveredResult, String>,
    (now, check_every_seconds): (DateTime<Utc>, Option<u64>),
) -> DiscoveredSet {
    let checked = attempt.and_then(|result| match discovered_problem(&result) {
        Some(problem) => Err(problem),
        None => discovered_digest(&result)
            .map(|digest| (result, digest))
            .map_err(|error| error.to_string()),
    });
    match checked {
        Ok((result, digest)) => DiscoveredSet {
            status: DiscoveredSetStatus::Ok,
            fetched_at: Some(now),
            last_good_at: Some(now),
            digest: Some(digest),
            error: None,
            check_every_seconds,
            result: Some(result),
        },
        Err(error) => DiscoveredSet {
            status: DiscoveredSetStatus::Failed,
            fetched_at: Some(now),
            last_good_at: previous.and_then(|set| set.last_good_at),
            digest: previous.and_then(|set| set.digest.clone()),
            error: Some(error),
            check_every_seconds,
            result: previous.and_then(|set| set.result.clone()),
        },
    }
}
