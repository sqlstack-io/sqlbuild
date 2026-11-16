//! An `EventExportSeverity` or its string value, as `EventExportSeverity(value)` accepts it.

use crate::event_exporting::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_core::event_export::models::ExportSeverity;

/// An `EventExportSeverity` or its string value, as `EventExportSeverity(value)` accepts it.
#[inline]
pub fn severity_from(value: &Bound<'_, PyAny>) -> PyResult<ExportSeverity> {
    values::severity_from(value)
}
