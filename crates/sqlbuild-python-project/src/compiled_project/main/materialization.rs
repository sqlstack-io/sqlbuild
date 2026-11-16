//! The `materialized` config value when it is text, `table` when it is absent.

use crate::compiled_project::_helpers::project;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// The `materialized` config value when it is text, `table` when it is absent.
#[inline]
pub fn materialization(values: &Bound<'_, PyDict>) -> PyResult<Option<String>> {
    project::materialization(values)
}
