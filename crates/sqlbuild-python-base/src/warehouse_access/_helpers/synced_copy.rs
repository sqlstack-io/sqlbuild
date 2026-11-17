//! A plan preview's adapter: warehouse reads come from the location's mirrored copy, nothing connects.

use pyo3::PyErr;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyModuleMethods;

use pyo3::create_exception;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyResult, Python, pyclass, pymethods};
use serde::Deserialize;

create_exception!(
    sqlbuild._native,
    WarehouseAccessRefusedError,
    PyRuntimeError,
    "Raised when a plan preview reaches past the synced copy for the warehouse."
);

/// Adapter methods that only describe the dialect, never reaching the warehouse.
pub(crate) const PASSTHROUGH_NAMES: [&str; 12] = [
    "_render_source_relation_cast_subquery_with_columns",
    "adapter_name",
    "default_database",
    "default_schema",
    "default_table_promotion_mode",
    "expression_inference_profile",
    "metadata_inspection_concurrency",
    "python_functions_inherit_default_namespace",
    "requires_derived_table_aliases",
    "sql_analysis_dialect",
    "sql_analysis_dialect_name",
    "star_exclude_keyword",
];
/// Rendering and capability methods pass through; every other unknown attribute is refused.
pub(crate) const PASSTHROUGH_PREFIXES: [&str; 2] = ["render_", "supports_"];
/// Local table holding the copy's fingerprints; previews read it through per-location views.
pub(crate) const FINGERPRINT_STORE: &str = "__sqlbuild_synced_fingerprints";
pub(crate) const FINGERPRINT_STORE_COLUMNS: [&str; 2] = ["state_database", "state_schema"];
pub(crate) const MODEL_NODE_TYPE: &str = "model";

/// Items a preview reports as known only once the run plans live.
pub(crate) const PREVIEW_UNKNOWNS: [&str; 4] = [
    "source freshness",
    "retention",
    "column rename support",
    "sources outside mirrored schemas",
];

/// A source's described columns, keyed to the exact SQL that was described.
#[derive(Deserialize)]
pub(crate) struct DescribedColumns {
    pub(crate) sql_hash: String,
    pub(crate) columns: Vec<String>,
}

pub(crate) fn refused(access: &str) -> pyo3::PyErr {
    WarehouseAccessRefusedError::new_err(format!(
        "plan previews read only the synced copy; refused warehouse access: {access}"
    ))
}

pub(crate) fn same(left: Option<&str>, right: Option<&str>) -> bool {
    left.unwrap_or_default().to_lowercase() == right.unwrap_or_default().to_lowercase()
}

/// Whether `wanted` is unset or holds `value`, compared as the warehouse folds case.
pub(crate) fn any_same(wanted: Option<&[String]>, value: &str) -> bool {
    let Some(wanted) = wanted else {
        return true;
    };
    for item in wanted {
        if same(Some(value), Some(item)) {
            return true;
        }
    }
    false
}

pub(crate) fn literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// A connection that refuses every use; like the adapter, it lives in the adapter contract's module.
#[pyclass(
    frozen,
    module = "sqlbuild.adapter.contract",
    name = "SyncedCopyConnection"
)]
pub(crate) struct SyncedCopyConnection;

#[pymethods]
impl SyncedCopyConnection {
    pub(crate) fn __getattr__(&self, name: &str) -> PyResult<()> {
        Err(refused(&format!("connection.{name}")))
    }
}

/// Register `WarehouseAccessRefusedError` on the native module.
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "WarehouseAccessRefusedError",
        module.py().get_type::<WarehouseAccessRefusedError>(),
    )
}

/// Whether `error` is a plan preview reaching past the synced copy for the warehouse.
pub(crate) fn is_warehouse_access_refused(py: Python<'_>, error: &PyErr) -> bool {
    error.is_instance_of::<WarehouseAccessRefusedError>(py)
}
