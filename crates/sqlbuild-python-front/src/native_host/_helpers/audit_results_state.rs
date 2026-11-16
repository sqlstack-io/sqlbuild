//! Pruning of the audit results state table through the adapter.

use crate::native_host::main::state_table_name::state_table_name;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_core::constants::AUDIT_RESULTS_TABLE_NAME;
use sqlbuild_state::audit_results::main::prune_audit_results_sql::prune_audit_results_sql;

/// SQL deleting audit results beyond the newest `retain_versions` per audit binding.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, retain_versions))]
pub(crate) fn build_audit_results_prune_sql(
    database: &Bound<'_, PyAny>,
    schema: &Bound<'_, PyAny>,
    render_qualified_name: &Bound<'_, PyAny>,
    retain_versions: i64,
) -> PyResult<String> {
    Ok(state_table_name(
        render_qualified_name,
        (database, schema),
        AUDIT_RESULTS_TABLE_NAME,
    )?
    .map(|table| prune_audit_results_sql(&table, retain_versions))
    .unwrap_or_default())
}
