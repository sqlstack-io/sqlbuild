//! `build_effective_vars` before template expansion.

use crate::contracts::_helpers::effective_config::Pairs;
use crate::contracts::_helpers::effective_config::dictionary;
use crate::contracts::_helpers::effective_config::pairs;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_config::effective::main::effective_vars::effective_vars;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `build_effective_vars` before template expansion.
#[pyfunction]
pub fn effective_config_vars<'py>(
    py: Python<'py>,
    configs: (Bound<'py, PyAny>, Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    cli_vars: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let (project_config, local_config) = configs;
    let project: Pairs<'py> = pairs(&native_field(&project_config, "vars")?)?;
    let target: Pairs<'py> = if target_config.is_none() {
        Vec::new()
    } else {
        pairs(&native_field(target_config, "vars")?)?
    };
    let local: Pairs<'py> = pairs(&native_field(&local_config, "vars")?)?;
    let cli: Pairs<'py> = pairs(cli_vars)?;
    dictionary(py, effective_vars(&[&project, &target, &local, &cli]))
}
