//! The model's old-name view retention, the project's when unset, or `None` when disabled.

use crate::state_rows::constants::OLD_NAME_VIEW_CONFIG_KEY;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyBool;

/// The model's old-name view retention, the project's when unset, or `None` when disabled.
pub fn old_name_retention<'py>(
    config_values: &Bound<'py, PyAny>,
    project_retention: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = config_values.py();
    if !config_values.contains(OLD_NAME_VIEW_CONFIG_KEY)? {
        return Ok(project_retention.clone());
    }
    let value: Bound<'py, PyAny> = config_values.get_item(OLD_NAME_VIEW_CONFIG_KEY)?;
    if value.is(PyBool::new(py, false)) {
        return Ok(py.None().into_bound(py));
    }
    value.str()?.call_method0("strip")
}
