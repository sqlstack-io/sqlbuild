//! `build_effective_vars` before template expansion.

use crate::contracts::_helpers::effective_config;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// `build_effective_vars` before template expansion.
#[inline]
pub fn effective_config_vars<'py>(
    py: Python<'py>,
    configs: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    cli_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    effective_config::effective_config_vars(py, configs, target_config, cli_vars)
}
