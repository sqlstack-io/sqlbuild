//! Column renames validated natively, raised by Python as the attachment raises them.

use pyo3::prelude::{Bound, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods};
use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::model_validation::main::validate_column_migrations::validate_column_migrations;
use sqlbuild_model_config::model_validation::models::ColumnMigration;

const MATERIALIZED_KEY: &str = "materialized";
const MIGRATE_FROM_KEY: &str = "migrate_from";

/// The first column-rename problem for a model with these config values, if any.
pub(crate) fn column_migration_error(
    model_name: &str,
    values: &Bound<'_, PyDict>,
    columns: &[ColumnMigration],
) -> PyResult<Option<ConfigError>> {
    if columns.iter().all(|column| column.migrate_from.is_none()) {
        return Ok(None);
    }
    let materialized: Option<String> = match values.get_item(MATERIALIZED_KEY)? {
        None => None,
        Some(value) if value.is_none() => None,
        Some(value) => match value.downcast::<PyString>() {
            Ok(text) => Some(text.to_str()?.to_owned()),
            Err(_) => return Ok(Some(ConfigError::config_value_type(MATERIALIZED_KEY))),
        },
    };
    let model_migrates: bool = values
        .get_item(MIGRATE_FROM_KEY)?
        .is_some_and(|value| !value.is_none());
    Ok(
        validate_column_migrations(model_name, materialized.as_deref(), model_migrates, columns)
            .err(),
    )
}
