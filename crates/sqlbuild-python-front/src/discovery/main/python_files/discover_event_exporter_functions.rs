//! Validated lifecycle-event sink declarations under sinks/, bound to providers; Recall replacement sinks are left out.

use crate::discovery::_helpers::python_files::imported_modules::{ProjectDir, tuple_of};
use crate::discovery::_helpers::python_files::imported_providers::bind_event_exporter_declarations;
use crate::discovery::_helpers::python_files::imported_providers::discover_event_exporter_declarations;
use crate::discovery::_helpers::python_files::imported_providers::takes_recall_writer;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;

/// Validated lifecycle-event sink declarations under sinks/, bound to providers; Recall replacement sinks are left out.
#[pyfunction]
#[pyo3(signature = (*, project_dir, providers = None))]
pub fn discover_event_exporter_functions<'py>(
    project_dir: Bound<'py, PyAny>,
    providers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project = ProjectDir {
        path: project_dir.clone(),
    };
    if let Some(cached) = project.cached_extensions()? {
        return cached.get_item(1);
    }
    let mut ordinary: Vec<Bound<'py, PyAny>> = Vec::new();
    for declaration in discover_event_exporter_declarations(project_dir.clone())?.try_iter()? {
        let declaration: Bound<'py, PyAny> = declaration?;
        if !takes_recall_writer(&declaration)? {
            ordinary.push(declaration);
        }
    }
    bind_event_exporter_declarations(
        tuple_of(py, ordinary)?,
        providers.map_or_else(|| tuple_of(py, Vec::new()), Ok)?,
        project_dir,
    )
}
