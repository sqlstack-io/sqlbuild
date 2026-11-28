//! The layers a target's writer or catalog connection merges from, read off the loaded config objects.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_config::effective::constants::CONNECTION_KIND_KEY;
use sqlbuild_config::effective::models::ConnectionLayers;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::contracts::_helpers::effective_config::{Key, Pairs, Value, named, optional_key, pairs};

/// The selected connection's layers, and whether they are the catalog's own (no writer layers).
pub(crate) fn connection_layers<'py>(
    (project_config, local_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    catalog: bool,
) -> PyResult<(ConnectionLayers<Key<'py>, Value<'py>>, bool)> {
    let catalog_name = if catalog && !target_config.is_none() {
        optional_key(&native_field(target_config, "catalog_connection")?)?
    } else {
        None
    };
    let independent = catalog_name.is_some();
    let target: Option<(Option<Key<'py>>, Pairs<'py>)> = if independent {
        Some((catalog_name, Vec::new()))
    } else if target_config.is_none() {
        None
    } else {
        Some((
            optional_key(&native_field(target_config, "connection_name")?)?,
            pairs(&native_field(target_config, "connection")?)?,
        ))
    };
    Ok((
        ConnectionLayers {
            project: writer_layer(project_config, independent)?,
            target,
            project_named: named(&native_field(project_config, "connections")?)?,
            local_named: named(&native_field(local_config, "connections")?)?,
            local: writer_layer(local_config, independent)?,
        },
        independent,
    ))
}

/// A config file's top-level `[connection]`, which a catalog's own connection leaves out.
fn writer_layer<'py>(config: &Bound<'py, PyAny>, independent: bool) -> PyResult<Pairs<'py>> {
    if independent {
        return Ok(Vec::new());
    }
    pairs(&native_field(config, "connection")?)
}

/// `connection` without its `kind`, which names the system and never reaches a warehouse driver.
pub(crate) fn without_kind(connection: Pairs<'_>) -> Pairs<'_> {
    connection
        .into_iter()
        .filter(|(key, _)| key.text() != CONNECTION_KIND_KEY)
        .collect()
}
