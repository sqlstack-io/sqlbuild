//! The project's providers and what uses them, published from the classes alone, so setting values never leave the location.

use std::collections::BTreeMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use serde_json::{Value, json};
use sqlbuild_orchestration::snapshots::models::SnapshotNode;
use sqlbuild_python_base::records::main::fields::field::field;

use crate::project_snapshot::_helpers::snapshot::snapshot_providers::{
    provider_consumers, provider_row,
};

/// Each provider's name, class, description, file, setting fields, consumers and the snapshot nodes that run them; `None` when the project declares none.
pub fn providers_document(
    discovered_inputs: &Bound<'_, PyAny>,
    nodes: &BTreeMap<String, SnapshotNode>,
) -> PyResult<Option<Value>> {
    let providers = field(discovered_inputs, "providers")?;
    if providers.len()? == 0 {
        return Ok(None);
    }
    let (mut used_by, mut running) = provider_consumers(discovered_inputs, nodes)?;
    let mut rows: Vec<Value> = Vec::new();
    for provider in providers.try_iter()? {
        let provider = provider?;
        let name: String = field(&provider, "name")?.extract()?;
        rows.push(provider_row(
            &provider,
            running.remove(&name).unwrap_or_default(),
            used_by.remove(&name).unwrap_or_default(),
        )?);
    }
    Ok(Some(json!({ "providers": rows })))
}
