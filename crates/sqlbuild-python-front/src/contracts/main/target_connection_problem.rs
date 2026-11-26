//! Whether each connection a target writes or reads its catalog through can hold models.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyString, PyStringMethods};
use sqlbuild_config::effective::constants::CONNECTION_KIND_KEY;
use sqlbuild_config::effective::main::effective_connection::effective_connection;
use sqlbuild_config::effective::main::warehouse_connection_problem::warehouse_connection_problem;
use sqlbuild_config::effective::models::WarehouseConnection;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::contracts::_helpers::connection_layers::connection_layers;
use crate::contracts::_helpers::effective_config::{Key, Pairs};
use crate::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;

/// Why `target`'s writer or catalog connection can't hold models, or `None` when both can.
pub fn target_connection_problem(
    (project_config, local_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    (target, resolved): (&str, &Bound<'_, PyAny>),
) -> PyResult<Option<String>> {
    let adapter: String = resolve_effective_adapter_name(project_config, local_config)?
        .str()?
        .to_string();
    for catalog in [false, true] {
        if catalog && native_field(resolved, "catalog_connection")?.is_none() {
            continue;
        }
        let (layers, _) = connection_layers((project_config, local_config), resolved, catalog)?;
        let name: Option<String> = layers
            .target
            .as_ref()
            .and_then(|(name, _)| name.as_ref().map(Key::text));
        let Ok(merged) = effective_connection(layers) else {
            continue;
        };
        let (kind, keys): (Option<String>, Vec<String>) = kind_and_keys(&merged)?;
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        let problem: Option<String> = warehouse_connection_problem(&WarehouseConnection {
            target,
            name: name.as_deref(),
            adapter: &adapter,
            kind: kind.as_deref(),
            keys: &keys,
        });
        if problem.is_some() {
            return Ok(problem);
        }
    }
    Ok(None)
}

/// The merged connection's `kind` as text, when it sets one, and every key it sets.
fn kind_and_keys(merged: &Pairs<'_>) -> PyResult<(Option<String>, Vec<String>)> {
    let mut kind: Option<String> = None;
    let mut keys: Vec<String> = Vec::with_capacity(merged.len());
    for (key, value) in merged {
        let key: String = key.text();
        if key == CONNECTION_KIND_KEY {
            kind = Some(match value.downcast::<PyString>() {
                Ok(text) => text.to_string_lossy().into_owned(),
                Err(_) => value.repr()?.to_string(),
            });
        }
        keys.push(key);
    }
    Ok((kind, keys))
}
