//! Computed attributes of the natively hosted project configuration records.

use pyo3::prelude::{Bound, Py, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `SettingsConfig.sql_validation`: the legacy name of the unified SQL analysis gate.
pub(crate) fn settings_sql_validation(instance: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    Ok(native_field(instance, "sql_analysis")?.unbind())
}
