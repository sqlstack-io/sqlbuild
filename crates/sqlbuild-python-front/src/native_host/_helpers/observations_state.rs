//! DDL and pruning of the display-only observations table through the adapter.

use crate::native_host::main::fixed_state_table_sql::fixed_state_table_sql;
use crate::native_host::main::state_table_name::state_table_name;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_core::constants::OBSERVATIONS_TABLE_NAME;
use sqlbuild_state::observations::constants::{
    NULLABLE_OBSERVATION_COLUMNS, OBSERVATION_COLUMN_TYPES,
};
use sqlbuild_state::observations::main::prune_observations_sql::prune_observations_sql;

/// Create-if-missing DDL for the observations table, with adapter-rendered column types.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, render_framework_type))]
pub(crate) fn build_observations_create_table_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    render_framework_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    fixed_state_table_sql(
        (database, schema),
        (render_qualified_name, render_framework_type),
        (
            OBSERVATIONS_TABLE_NAME,
            &OBSERVATION_COLUMN_TYPES,
            &NULLABLE_OBSERVATION_COLUMNS,
        ),
    )
}

/// SQL deleting observations beyond the newest `retain_versions` per kind and node.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, retain_versions))]
pub(crate) fn build_observations_prune_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    retain_versions: i64,
) -> PyResult<String> {
    Ok(state_table_name(
        render_qualified_name,
        (database, schema),
        OBSERVATIONS_TABLE_NAME,
    )?
    .map(|table| prune_observations_sql(&table, retain_versions))
    .unwrap_or_default())
}
