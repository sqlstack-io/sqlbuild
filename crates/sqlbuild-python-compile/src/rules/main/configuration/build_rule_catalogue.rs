//! The validated built-in and custom rule catalogue sorted by code.

use crate::rules::_helpers::configuration::catalogue::rule_catalogue;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyTuple;

/// The validated built-in and custom rule catalogue sorted by code.
#[pyfunction]
#[pyo3(signature = (*, config, project_dir, include_custom = true))]
pub fn build_rule_catalogue<'py>(
    config: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
    include_custom: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(
        config.py(),
        rule_catalogue(config, project_dir, include_custom)?,
    )
}
