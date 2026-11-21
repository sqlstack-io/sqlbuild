//! One discover function called again, as a location's `check_every` does, without publishing.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_orchestration::discovered_sets::models::DiscoveredSet;
use sqlbuild_orchestration::discovered_sets::types::DiscoveredSets;

use crate::discovered_sets::_helpers::live_sets::live_set;
use crate::discovered_sets::models::{LiveDiscovery, Rediscover};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Set `name` after calling its function again; `None` when no discover function has that name.
pub fn rediscover_set(
    inputs: &Bound<'_, PyAny>,
    (name, previous): (&str, Option<DiscoveredSet>),
    target: Option<String>,
) -> PyResult<Option<DiscoveredSet>> {
    for record in native_field(inputs, "discover_functions")?.try_iter()? {
        let record: Bound<'_, PyAny> = record?;
        if native_field(&record, "name")?.extract::<String>()? != name {
            continue;
        }
        let check_every: Option<u64> = native_field(&record, "check_every_seconds")?.extract()?;
        let live = LiveDiscovery {
            target,
            previous: previous
                .map(|set| DiscoveredSets::from([(name.to_owned(), set)]))
                .unwrap_or_default(),
            rediscover: Rediscover::All,
            keep_last_good: true,
            seed: None,
        };
        return live_set(inputs, (&record, name, check_every), (&live, true)).map(Some);
    }
    Ok(None)
}
