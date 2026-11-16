//! The export policy of a validated event's type; `KeyError` for an uncatalogued type.

use crate::event_exporting::_helpers::values;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_core::event_export::models::ExportPolicy;

/// The export policy of a validated event's type; `KeyError` for an uncatalogued type.
#[inline]
pub fn export_policy(py: Python<'_>, event_type: &str) -> PyResult<ExportPolicy> {
    values::export_policy(py, event_type)
}
