//! Merged effective vars with recursive `${name}` expansion.

use crate::model_config::_helpers::config::template_data;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Merged effective vars with recursive `${name}` expansion.
#[inline]
pub fn expanded_effective_vars<'py>(
    raw_values: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    template_data::expanded_effective_vars(raw_values)
}
