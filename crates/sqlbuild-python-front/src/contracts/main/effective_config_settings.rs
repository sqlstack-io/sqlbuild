//! `build_effective_settings`: setting values by field name, local overrides applied.

use crate::contracts::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;
use pyo3::types::PyString;

/// `build_effective_settings`: setting values by field name, local overrides applied.
#[inline]
pub fn effective_config_settings<'py>(
    py: Python<'py>,
    settings: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    overrides: &Bound<'py, PyAny>,
    names: Vec<Bound<'py, PyString>>,
) -> PyResult<Bound<'py, PyDict>> {
    effective_config::effective_config_settings(py, settings, overrides, names)
}
