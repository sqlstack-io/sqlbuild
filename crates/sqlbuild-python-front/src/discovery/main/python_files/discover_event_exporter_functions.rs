//! Validated lifecycle-event sink declarations under sinks/, bound to providers; Recall replacement sinks are left out.

use crate::discovery::_helpers::python_files::imported_providers;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Validated lifecycle-event sink declarations under sinks/, bound to providers; Recall replacement sinks are left out.
#[inline]
pub fn discover_event_exporter_functions<'py>(
    project_dir: Bound<'py, PyAny>,
    providers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    imported_providers::discover_event_exporter_functions(project_dir, providers)
}
