//! The Python `EventExportSeverity` member of `severity`.

use crate::event_exporting::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_core::event_export::models::ExportSeverity;

/// The Python `EventExportSeverity` member of `severity`.
#[inline]
pub fn severity_object(py: Python<'_>, severity: ExportSeverity) -> PyResult<Bound<'_, PyAny>> {
    values::severity_object(py, severity)
}
