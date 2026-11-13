//! Pruning of the audit results state table through the adapter.

use pyo3::prelude::{Bound, PyAny, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::constants::AUDIT_RESULTS_TABLE_NAME;
use sqlbuild_state::audit_results::main::prune_audit_results_sql::prune_audit_results_sql;

use crate::bindings::_helpers::native_host::consumption_state::state_table_name;

/// SQL deleting audit results beyond the newest `retain_versions` per audit binding.
#[pyfunction]
#[pyo3(signature = (*, database, schema, render_qualified_name, retain_versions))]
fn build_audit_results_prune_sql(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_audit_results_prune_sql, module)?)
}
