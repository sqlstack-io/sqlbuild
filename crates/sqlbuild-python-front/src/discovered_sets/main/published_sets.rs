//! The sets a location runtime's publication used, kept for its later compiles.

use std::collections::BTreeMap;

use pyo3::prelude::{Bound, PyAny, PyResult};
use serde_json::{Value, json};

use crate::discovered_sets::_helpers::discover_files::discover_files;
use crate::discovered_sets::main::set_discovery_source::set_discovery_source;
use crate::discovered_sets::main::used_discovered_sets::used_discovered_sets;
use crate::discovered_sets::models::DiscoverySource;

/// After a publication, its sets for later compiles and the reply, with each function's file.
pub fn published_sets(held: Option<&Bound<'_, PyAny>>) -> PyResult<(Value, Value)> {
    let sets = used_discovered_sets();
    set_discovery_source(DiscoverySource::Saved(sets.clone()));
    let files: BTreeMap<String, String> = match held {
        Some(discovered) => discover_files(discovered)?,
        None => BTreeMap::new(),
    };
    Ok((json!(sets), json!(files)))
}
