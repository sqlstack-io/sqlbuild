//! The compile's retained project, or one recorded from a hand-built project.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::_helpers::project::records::hand_built_project;
use crate::compiled_views::models::NativeProjectView;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The compile's retained project, or one recorded from a hand-built project.
#[pyfunction]
pub fn compiled_project_facts<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, NativeCompiledProject>> {
    if let Ok(view) = project.cast::<NativeProjectView>() {
        return Ok(view.get().owner().bind(project.py()).clone());
    }
    let retained: Bound<'py, PyAny> = native_field(project, "native_project")?;
    if !retained.is_none() {
        return Ok(retained.cast_into()?);
    }
    hand_built_project(project)
}
