//! Scoped declarations the test file at `path` uses that resolve differently from `destination`.

use crate::refactor::_helpers::refactor_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Scoped declarations the test file at `path` uses that resolve differently from `destination`.
#[inline]
pub fn unit_test_rebindings(
    project: &Bound<'_, PyAny>,
    paths: (&str, &str),
) -> PyResult<Option<Vec<String>>> {
    refactor_facts::unit_test_rebindings(project, paths)
}
