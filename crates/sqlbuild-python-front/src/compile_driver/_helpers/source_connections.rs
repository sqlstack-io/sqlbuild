//! Source connections: the `[connections.*]` entries with a `kind` that no target, catalog or Recall uses, checked by SQLLoad.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyFloat, PyInt, PyList, PyString, PyTuple};
use sqlbuild_config::effective::constants::CONNECTION_KIND_KEY;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
use sqlbuild_load::source_connections::main::source_connections::source_connections;
use sqlbuild_python_base::records::main::construct::new_error::new_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;

use crate::compile_driver::models::{ConnectionRoles, ProjectConfiguration, SourceConnection};

const LOCAL_CONFIG_FILENAME: &str = "sqlbuild_local.toml";

/// Reject a source connection whose settings SQLLoad refuses, naming the file that defines it.
pub(crate) fn reject_invalid_source_connections(
    config: &ProjectConfiguration<'_, '_>,
) -> PyResult<()> {
    let py: Python<'_> = config.project_dir.py();
    for connection in connection_roles(config)?.sources {
        let mut table = toml::Table::new();
        table.insert(
            connection.name.clone(),
            toml::Value::Table(connection.settings),
        );
        if let Err(message) = source_connections(table) {
            let path: Bound<'_, PyAny> = config.project_dir.div(connection.file)?;
            let message: String = format!("{} {}", path.str()?, message.trim_end());
            return Err(new_error(
                py,
                ErrorId::ProjectConfig,
                &message,
                (None, None),
            ));
        }
    }
    Ok(())
}

/// The project's connections by use: each source connection's merged settings and defining file, and each SQLBuild connection.
pub(crate) fn connection_roles(config: &ProjectConfiguration<'_, '_>) -> PyResult<ConnectionRoles> {
    let used: Vec<String> = sqlbuild_connection_names(config)?;
    let project = native_field(config.project_config, "connections")?.downcast_into::<PyDict>()?;
    let local = native_field(config.local_config, "connections")?.downcast_into::<PyDict>()?;
    let mut names: Vec<String> = project.keys().extract()?;
    for name in local.keys().extract::<Vec<String>>()? {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    let mut roles = ConnectionRoles::default();
    for name in names {
        let entries: Vec<Bound<'_, PyDict>> = [project.get_item(&name)?, local.get_item(&name)?]
            .into_iter()
            .flatten()
            .map(|entry| entry.downcast_into::<PyDict>().map_err(PyErr::from))
            .collect::<PyResult<_>>()?;
        let mut settings = toml::Table::new();
        for entry in &entries {
            for (key, value) in entry.iter() {
                if let Some(value) = toml_value(&value)? {
                    settings.insert(key.extract()?, value);
                }
            }
        }
        let sets_kind: bool = settings.contains_key(CONNECTION_KIND_KEY);
        if used.contains(&name) || !sets_kind {
            roles.warehouses.push((name, sets_kind));
            continue;
        }
        let file: &'static str = match project.contains(&name)? {
            true => PROJECT_CONFIG_FILENAME,
            false => LOCAL_CONFIG_FILENAME,
        };
        roles.sources.push(SourceConnection {
            name,
            file,
            settings,
        });
    }
    Ok(roles)
}

/// Every connection a target, its catalog or Recall names in either file: SQLBuild connections, whatever their `kind`.
fn sqlbuild_connection_names(config: &ProjectConfiguration<'_, '_>) -> PyResult<Vec<String>> {
    let mut names: Vec<String> = Vec::new();
    for document in [config.project_config, config.local_config] {
        names.extend(native_field(document, "recall_connections")?.extract::<Vec<String>>()?);
        for target in native_field(document, "targets")?
            .downcast::<PyDict>()?
            .values()
        {
            for field in ["connection_name", "catalog_connection"] {
                let name: Bound<'_, PyAny> = native_field(&target, field)?;
                if !name.is_none() {
                    names.push(name.extract()?);
                }
            }
        }
    }
    Ok(names)
}

/// A loaded setting as TOML, `None` for an unset one; other values keep their text.
pub(crate) fn toml_value(value: &Bound<'_, PyAny>) -> PyResult<Option<toml::Value>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(if value.is_instance_of::<PyBool>() {
        toml::Value::Boolean(value.extract()?)
    } else if value.is_instance_of::<PyInt>() {
        match value.extract::<i64>() {
            Ok(number) => toml::Value::Integer(number),
            Err(_) => toml::Value::String(value.str()?.extract()?),
        }
    } else if value.is_instance_of::<PyFloat>() {
        toml::Value::Float(value.extract()?)
    } else if value.is_instance_of::<PyString>() {
        toml::Value::String(value.extract()?)
    } else if let Ok(items) = value.downcast::<PyDict>() {
        let mut table = toml::Table::new();
        for (key, item) in items.iter() {
            if let Some(item) = toml_value(&item)? {
                table.insert(key.str()?.extract()?, item);
            }
        }
        toml::Value::Table(table)
    } else if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        let mut items: Vec<toml::Value> = Vec::new();
        for item in value.try_iter()? {
            items.extend(toml_value(&item?)?);
        }
        toml::Value::Array(items)
    } else {
        toml::Value::String(value.str()?.extract()?)
    }))
}
