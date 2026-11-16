//! `str(Path(...).stem)` of the path attribute `name`.

use crate::discovery::_helpers::project::project_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `str(Path(...).stem)` of the path attribute `name`.
#[inline]
pub fn stem(owner: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    project_facts::stem(owner, name)
}
