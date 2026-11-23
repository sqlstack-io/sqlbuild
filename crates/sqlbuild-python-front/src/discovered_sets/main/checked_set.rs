//! A location runtime's `check_every` check of one discovered set.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_orchestration::discovered_sets::main::result_changed::result_changed;
use sqlbuild_orchestration::discovered_sets::models::{DiscoveredSetCheck, DiscoveredSetChecked};
use sqlbuild_python_base::boundary::models::KeywordArguments;

use crate::discovered_sets::_helpers::rediscover_set::rediscover_set;

/// Call one discover function again and say whether its result changed; nothing is published.
pub fn checked_set(
    arguments: &KeywordArguments<'_>,
    held: Option<&Bound<'_, PyAny>>,
    check: DiscoveredSetCheck,
) -> PyResult<DiscoveredSetChecked> {
    let Some(discovered) = held else {
        return Ok(DiscoveredSetChecked {
            set: None,
            changed: false,
        });
    };
    let target: String = arguments.value("selected_target")?.extract()?;
    let set = rediscover_set(
        discovered,
        (&check.check_discovered, check.previous.clone()),
        Some(target),
    )?;
    let changed: bool = set
        .as_ref()
        .is_some_and(|set| result_changed(check.previous.as_ref(), set));
    Ok(DiscoveredSetChecked { set, changed })
}
