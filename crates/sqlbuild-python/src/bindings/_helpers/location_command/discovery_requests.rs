//! A location runtime's discovered sets: live discovery when it publishes, and `check_every` checks.

use std::collections::BTreeMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyAnyMethods, PyResult};
use serde_json::{Value, json};
use sqlbuild_orchestration::discovered_sets::main::result_changed::result_changed;
use sqlbuild_orchestration::discovered_sets::models::{
    DiscoveredSetCheck, DiscoveredSetChecked, DiscoveryPublishRequest,
};
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_front::discovered_sets::main::discover_files::discover_files;
use sqlbuild_python_front::discovered_sets::main::rediscover_set::rediscover_set;
use sqlbuild_python_front::discovered_sets::main::set_discovery_source::set_discovery_source;
use sqlbuild_python_front::discovered_sets::main::used_discovered_sets::used_discovered_sets;
use sqlbuild_python_front::discovered_sets::models::{
    DiscoverySource, LiveDiscovery, Rediscover, SeedSource,
};

use crate::bindings::_helpers::location_command::types::HeldProject;

/// Discover live for the publication `request` asks for: its functions called again, the rest reused.
pub(super) fn discover_for_publication(
    arguments: &KeywordArguments<'_>,
    request: DiscoveryPublishRequest,
) -> PyResult<()> {
    let target: String = arguments.value("selected_target")?.extract()?;
    let location_id = arguments
        .value("location_id")?
        .extract::<String>()?
        .parse()
        .map_err(|_| PyValueError::new_err("location ID must be a UUID"))?;
    let seed: Option<SeedSource> = request.seed.then(|| SeedSource {
        server_url: arguments
            .value("server_url")
            .and_then(|url| url.extract())
            .unwrap_or_default(),
        location_id,
        target: target.clone(),
    });
    set_discovery_source(DiscoverySource::Live(LiveDiscovery {
        target: Some(target),
        previous: request.previous,
        rediscover: request.rediscover.map_or(Rediscover::All, Rediscover::Only),
        keep_last_good: true,
        seed,
    }));
    Ok(())
}

/// After a publication, its sets for later compiles and the reply, with each function's file.
pub(super) fn published_sets(held: Option<&HeldProject<'_>>) -> PyResult<(Value, Value)> {
    let sets = used_discovered_sets();
    set_discovery_source(DiscoverySource::Saved(sets.clone()));
    let files: BTreeMap<String, String> = match held {
        Some((discovered, _)) => discover_files(discovered)?,
        None => BTreeMap::new(),
    };
    Ok((json!(sets), json!(files)))
}

/// Call one discover function again and say whether its result changed; nothing is published.
pub(super) fn checked_set(
    arguments: &KeywordArguments<'_>,
    held: Option<&HeldProject<'_>>,
    check: DiscoveredSetCheck,
) -> PyResult<DiscoveredSetChecked> {
    let Some((discovered, _)) = held else {
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
