//! Setting guidance shown by diagnostics that depend on project settings.

use crate::discovery::main::failures::sql_analysis_guidance::sql_analysis_guidance;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyBoolMethods, PyInt, PyString};
use sqlbuild_config::project::main::snippet_help_text::snippet_help_text;
use sqlbuild_config::project::models::SettingText;

pub(crate) const SQL_ANALYSIS_KEY: &str = "sql_analysis";

#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, purpose))]
pub(crate) fn sql_analysis_off_guidance(
    discovered_inputs: &Bound<'_, PyAny>,
    purpose: &str,
) -> PyResult<Option<(String, String)>> {
    sql_analysis_guidance(discovered_inputs, purpose)
}

/// A Python `bool | int | str` setting value as help text shows it.
pub(crate) fn setting_text(value: &Bound<'_, PyAny>) -> PyResult<SettingText> {
    if let Ok(flag) = value.cast::<PyBool>() {
        return Ok(SettingText::Bool(flag.is_true()));
    }
    if value.is_instance_of::<PyInt>() {
        return Ok(SettingText::Integer(value.str()?.extract()?));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(SettingText::Text(value.extract()?));
    }
    Err(PyTypeError::new_err(
        "setting value must be a bool, int or str",
    ))
}

/// Render `<purpose>, <target>:` followed by the exact lines to write, one per line.
#[pyfunction]
#[pyo3(signature = (*, purpose, target, lines))]
pub(crate) fn snippet_help(purpose: &str, target: &str, lines: Vec<String>) -> String {
    snippet_help_text(purpose, target, &lines)
}
