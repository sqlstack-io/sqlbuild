//! The project's compiled models, in compile order.

use crate::compiled_views::_helpers::project::project_reads::MODELS_FIELD;
use crate::compiled_views::_helpers::project::project_reads::native_project;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The project's compiled models, in compile order.
pub fn compiled_models<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.models(py).into_bound(py).into_any());
    }
    native_field(project, MODELS_FIELD)
}
