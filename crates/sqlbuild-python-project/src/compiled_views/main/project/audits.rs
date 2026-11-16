//! The project's compiled audits.

use crate::compiled_views::_helpers::project::project_reads::AUDITS_FIELD;
use crate::compiled_views::_helpers::project::project_reads::native_project;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The project's compiled audits.
pub fn audits<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.audits(py).into_bound(py));
    }
    native_field(project, AUDITS_FIELD)
}
