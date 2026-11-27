//! Merge effective vars using the locked precedence order.

use crate::contracts::main::effective_config_vars::effective_config_vars;
use crate::model_config::main::config::expanded_effective_vars::expanded_effective_vars;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::pyfunction;

/// Merge effective vars using the locked precedence order.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, target_config, cli_vars))]
pub fn build_effective_vars<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
    cli_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_config.py();
    let merged: Bound<'py, PyAny> = effective_config_vars(
        py,
        (project_config.clone(), local_config.clone()),
        target_config,
        cli_vars,
    )
    .and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))?;
    expanded_effective_vars(merged.cast()?)
}
