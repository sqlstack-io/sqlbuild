//! Parallel resolution of the reads a batch of tests will look up.

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::collections::HashSet;

use crate::planner_fixtures::_helpers::resolved_reads::{stored, uncached_reads};
use crate::planner_fixtures::models::{FixtureReadCache, FixtureReadKey};
use crate::required_columns::errors::RequiredColumnsError;
use crate::required_columns::models::ColumnRead;

/// Resolve the uncached keys in parallel and keep the successful reads; failures stay uncached.
pub fn prewarm_reads(mut cache: FixtureReadCache, keys: &[FixtureReadKey]) -> FixtureReadCache {
    let mut seen: HashSet<&FixtureReadKey> = HashSet::new();
    let missing: Vec<&FixtureReadKey> = keys
        .iter()
        .filter(|key| !cache.entries.contains_key(*key) && seen.insert(*key))
        .collect();
    let resolved: Vec<(
        &FixtureReadKey,
        Result<Vec<ColumnRead>, RequiredColumnsError>,
    )> = missing
        .par_iter()
        .map(|key| (*key, uncached_reads(key)))
        .collect();
    for (key, reads) in resolved {
        cache.misses += 1;
        let Ok(reads) = reads else {
            continue;
        };
        cache = stored(cache, key, reads);
    }
    cache
}
