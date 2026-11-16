//! The first column-rename problem for a model with these config values, if any.

use crate::model_config::_helpers::model_nodes::column_migrations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;
use sqlbuild_model_config::errors::ConfigError;
use sqlbuild_model_config::model_validation::models::ColumnMigration;

/// The first column-rename problem for a model with these config values, if any.
#[inline]
pub fn column_migration_error(
    model_name: &str,
    values: &Bound<'_, PyDict>,
    columns: &[ColumnMigration],
) -> PyResult<Option<ConfigError>> {
    column_migrations::column_migration_error(model_name, values, columns)
}
