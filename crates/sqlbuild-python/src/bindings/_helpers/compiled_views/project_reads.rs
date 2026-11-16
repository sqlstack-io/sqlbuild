//! A compiled project's resource collections, read from its native view.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;
use sqlbuild_python_base::compile_driver::constants::COMPILED_VIEW_FALLBACKS;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const MODELS_FIELD: &str = "models";
const SOURCES_FIELD: &str = "sources";
const SEEDS_FIELD: &str = "seeds";
const FUNCTIONS_FIELD: &str = "functions";
const AUDITS_FIELD: &str = "audits";

/// The native view behind a project object; any other object is a Python-built record, counted.
#[inline]
fn native_project<'a>(project: &'a Bound<'_, PyAny>) -> PyResult<Option<&'a NativeProjectView>> {
    if let Ok(view) = project.cast::<NativeProjectView>() {
        return Ok(Some(view.get()));
    }
    count_stage(project.py(), COMPILED_VIEW_FALLBACKS, 1)?;
    Ok(None)
}

/// The project's compiled models, in compile order.
pub(crate) fn models<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.models(py).into_bound(py).into_any());
    }
    native_field(project, MODELS_FIELD)
}

/// The project's compiled sources.
pub(crate) fn sources<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.sources(py).into_bound(py));
    }
    native_field(project, SOURCES_FIELD)
}

/// The project's compiled functions.
pub(crate) fn functions<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.functions(py).into_bound(py));
    }
    native_field(project, FUNCTIONS_FIELD)
}

/// The project's compiled seeds.
pub(crate) fn seeds<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.seeds(py).into_bound(py));
    }
    native_field(project, SEEDS_FIELD)
}

/// The project's compiled audits.
pub(crate) fn audits<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.audits(py).into_bound(py));
    }
    native_field(project, AUDITS_FIELD)
}

/// The project's resource collection named `field`; any other field is read by attribute.
pub(crate) fn collection<'py>(
    project: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Bound<'py, PyAny>> {
    match field {
        MODELS_FIELD => models(project),
        SOURCES_FIELD => sources(project),
        SEEDS_FIELD => seeds(project),
        FUNCTIONS_FIELD => functions(project),
        AUDITS_FIELD => audits(project),
        _ => native_field(project, field),
    }
}

/// The project's compiled SQL tests.
pub(crate) fn sql_tests<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.sql_tests(py).into_bound(py));
    }
    native_field(project, "sql_tests")
}
