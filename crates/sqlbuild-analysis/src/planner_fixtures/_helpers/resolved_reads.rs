//! Bounded reuse of native compiler read resolution.

use crate::planner_fixtures::models::{FixtureReadCache, FixtureReadKey};
use crate::required_columns::errors::RequiredColumnsError;
use crate::required_columns::main::resolve_required_columns::resolve_required_columns;
use crate::required_columns::models::{ColumnRead, ReferenceInput};

/// Cache successful read resolution by the full original Python cache identity.
pub(crate) fn resolved_reads(
    mut cache: FixtureReadCache,
    key: &FixtureReadKey,
) -> (
    FixtureReadCache,
    Result<Vec<ColumnRead>, RequiredColumnsError>,
) {
    cache.clock += 1;
    let clock = cache.clock;
    if let Some((used, reads)) = cache.entries.get_mut(key) {
        cache.recency.remove(used);
        cache.recency.insert(clock, key.clone());
        *used = clock;
        let reads = reads.clone();
        return (cache, Ok(reads));
    }
    cache.misses += 1;
    let reads = match uncached_reads(key) {
        Ok(reads) => reads,
        Err(error) => return (cache, Err(error)),
    };
    (stored(cache, key, reads.clone()), Ok(reads))
}

/// Resolve one key's reads without the cache.
pub(crate) fn uncached_reads(
    key: &FixtureReadKey,
) -> Result<Vec<ColumnRead>, RequiredColumnsError> {
    let references: Vec<ReferenceInput> = key
        .references
        .iter()
        .map(|reference| ReferenceInput {
            kind: reference.kind.clone(),
            name: reference.name.clone(),
        })
        .collect();
    resolve_required_columns(&key.sql, &references, key.dialect.as_deref())
}

/// Keep one key's successful reads, evicting the least recently used entry at capacity.
pub(crate) fn stored(
    mut cache: FixtureReadCache,
    key: &FixtureReadKey,
    reads: Vec<ColumnRead>,
) -> FixtureReadCache {
    if cache.entries.len() >= crate::planner_fixtures::constants::READ_CACHE_CAPACITY
        && let Some((_, oldest)) = cache.recency.pop_first()
    {
        cache.entries.remove(&oldest);
    }
    cache.clock += 1;
    let clock = cache.clock;
    cache.recency.insert(clock, key.clone());
    cache.entries.insert(key.clone(), (clock, reads));
    cache
}
