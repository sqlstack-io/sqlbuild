//! A `@loader` as SQLLoad's check reads it: its options, connection, and the table and columns it fills.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_load::loads::models::LoaderDeclaration;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

use crate::compile_driver::_helpers::source_connections::toml_value;

/// `loader` as SQLLoad's check reads it, and the file it is shown by.
pub(crate) fn loader_declaration(
    loader: &Bound<'_, PyAny>,
    source_files: &Bound<'_, PyAny>,
) -> PyResult<(LoaderDeclaration, String)> {
    let name: String = native_field(loader, "name")?.extract()?;
    let module: String = native_field(loader, "relative_path")?.str()?.to_string();
    let mut settings = toml::Table::new();
    for (key, value) in native_field(loader, "settings")?.cast::<PyDict>()?.iter() {
        if let Some(value) = toml_value(&value)? {
            settings.insert(key.str()?.to_string(), value);
        }
    }
    let (table, columns): (String, Vec<(String, String)>) =
        match filled_source(source_files, &name)? {
            Some(entry) => {
                let table: Option<String> = native_field(&entry, "table")?.extract()?;
                (
                    table.unwrap_or_else(|| name.clone()),
                    typed_columns(&entry)?,
                )
            }
            None => {
                let destination: Option<String> = native_field(loader, "destination")?.extract()?;
                let table: String = destination
                    .and_then(|destination| destination.rsplit('.').next().map(str::to_owned))
                    .unwrap_or_else(|| name.clone());
                (table, typed_columns(loader)?)
            }
        };
    Ok((
        LoaderDeclaration {
            name,
            module: module.clone(),
            connection: native_field(loader, "connection")?.extract()?,
            settings,
            columns,
            table,
        },
        module,
    ))
}

/// The source entry the loader `name` fills, if a sources file lists one.
fn filled_source<'py>(
    source_files: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    for file in source_files.try_iter()? {
        for entry in native_field(&file?, "source_entries")?.try_iter()? {
            let entry: Bound<'py, PyAny> = entry?;
            if native_field(&entry, "loader")?
                .extract::<Option<String>>()?
                .as_deref()
                == Some(name)
            {
                return Ok(Some(entry));
            }
        }
    }
    Ok(None)
}

/// Each column of `owner`'s `columns` with a declared `type`.
fn typed_columns(owner: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut columns: Vec<(String, String)> = Vec::new();
    for column in native_field(owner, "columns")?.try_iter()? {
        let column: Bound<'_, PyAny> = column?;
        if let Some(kind) = native_field(&column, "type")?.extract::<Option<String>>()? {
            columns.push((native_field(&column, "name")?.extract()?, kind));
        }
    }
    Ok(columns)
}
