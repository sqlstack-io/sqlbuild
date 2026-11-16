//! The compile's retained project, or one recorded from a hand-built project.

use crate::compiled_project::models::NativeCompiledProject;
use crate::compiled_views::_helpers::project::records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The compile's retained project, or one recorded from a hand-built project.
#[inline]
pub fn compiled_project_facts<'py>(
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, NativeCompiledProject>> {
    records::compiled_project_facts(project)
}
