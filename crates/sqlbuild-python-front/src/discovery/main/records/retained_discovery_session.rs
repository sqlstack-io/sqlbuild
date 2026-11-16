//! The session this discovery pass read its declaration files with, if any.

use crate::discovery::_helpers::records::declaration_records;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The session this discovery pass read its declaration files with, if any.
#[inline]
pub fn retained_discovery_session<'py>(
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    declaration_records::retained_discovery_session(project_dir)
}
