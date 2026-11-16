//! The project's compiled SQL tests.

use crate::compiled_views::_helpers::project::project_reads::native_project;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The project's compiled SQL tests.
pub fn sql_tests<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    if let Some(view) = native_project(project)? {
        return Ok(view.sql_tests(py).into_bound(py));
    }
    native_field(project, "sql_tests")
}
