//! What a load's compile check reads: the project's connections and warehouse, and one file's load and the sources it fills.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_load::loads::models::{FilledSource, LoadCheck, LoadDeclaration};
use sqlbuild_load::warehouses::main::load_warehouse::load_warehouse;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::_helpers::source_connections::{connection_roles, toml_value};
use crate::compile_driver::models::{ConnectionRoles, ProjectConfiguration};
use crate::contracts::main::resolve_effective_adapter_name::resolve_effective_adapter_name;

/// The key of a load's own name, which SQLBuild reads and SQLLoad does not.
const LOAD_NAME_KEY: &str = "name";

/// The project's source connections and warehouse, or the error naming a warehouse SQLLoad can't load into.
pub(crate) fn load_check(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<Result<LoadCheck, String>> {
    let adapter: String =
        resolve_effective_adapter_name(config.project_config, config.local_config)?
            .str()?
            .to_string();
    let (warehouse, motherduck) = match load_warehouse(&adapter) {
        Ok(found) => found,
        Err(message) => return Ok(Err(message)),
    };
    let roles: ConnectionRoles = connection_roles(config)?;
    let mut connections = toml::Table::new();
    for connection in roles.sources {
        connections.insert(connection.name, toml::Value::Table(connection.settings));
    }
    Ok(Ok(LoadCheck {
        connections,
        warehouse_connections: roles.warehouses,
        warehouse,
        motherduck,
    }))
}

/// The load of one sources file and the sources it fills there, as SQLLoad's check reads them.
pub(crate) fn declaration(
    file: &Bound<'_, PyAny>,
    load: &Bound<'_, PyAny>,
) -> PyResult<LoadDeclaration> {
    let name: String = native_field(load, "name")?.extract()?;
    let mut settings = toml::Table::new();
    for (key, value) in native_field(load, "settings")?.cast::<PyDict>()?.iter() {
        let key: String = key.str()?.to_string();
        if key == LOAD_NAME_KEY {
            continue;
        }
        if let Some(value) = toml_value(&value)? {
            settings.insert(key, value);
        }
    }
    let mut sources: Vec<FilledSource> = Vec::new();
    for entry in native_field(file, "source_entries")?.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        if native_field(&entry, "load")?
            .extract::<Option<String>>()?
            .as_deref()
            != Some(&name)
        {
            continue;
        }
        let mut columns: Vec<(String, String)> = Vec::new();
        for column in native_field(&entry, "columns")?.try_iter()? {
            let column: Bound<'_, PyAny> = column?;
            if let Some(kind) = native_field(&column, "type")?.extract::<Option<String>>()? {
                columns.push((native_field(&column, "name")?.extract()?, kind));
            }
        }
        sources.push(FilledSource {
            name: native_field(&entry, "name")?.extract()?,
            columns,
        });
    }
    Ok(LoadDeclaration {
        name,
        settings,
        sources,
    })
}
