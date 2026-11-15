//! Setting guidance shown by diagnostics that depend on project settings.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyBoolMethods, PyDict, PyInt, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::project::main::explicit_references_help::explicit_references_help as references_help;
use sqlbuild_config::project::main::microbatch_guidance::microbatch_guidance as concurrency_guidance;
use sqlbuild_config::project::main::setting_help_text::setting_help_text;
use sqlbuild_config::project::main::snippet_help_text::snippet_help_text;
use sqlbuild_config::project::main::sql_analysis_off_guidance::sql_analysis_off_guidance as analysis_off;
use sqlbuild_config::project::models::{SettingSite, SettingText};

use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
const SQL_ANALYSIS_KEY: &str = "sql_analysis";

/// The note and help for turning SQL analysis on, or `None` when it is effectively on.
pub(crate) fn sql_analysis_guidance(
    discovered_inputs: &Bound<'_, PyAny>,
    purpose: &str,
) -> PyResult<Option<(String, String)>> {
    let local: Bound<'_, PyAny> = native_field(discovered_inputs, "local_config")?;
    let local_override: bool =
        native_field(&local, "setting_overrides")?.contains(SQL_ANALYSIS_KEY)?;
    let owner: Bound<'_, PyAny> = if local_override {
        local
    } else {
        native_field(discovered_inputs, "project_config")?
    };
    if native_field(&native_field(&owner, "settings")?, SQL_ANALYSIS_KEY)?.is_truthy()? {
        return Ok(None);
    }
    Ok(Some(analysis_off(local_override, purpose)))
}

#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, purpose))]
fn sql_analysis_off_guidance(
    discovered_inputs: &Bound<'_, PyAny>,
    purpose: &str,
) -> PyResult<Option<(String, String)>> {
    sql_analysis_guidance(discovered_inputs, purpose)
}

#[pyfunction]
pub(crate) fn microbatch_guidance() -> (String, String) {
    concurrency_guidance()
}

#[pyfunction]
#[pyo3(signature = (*, allowed))]
pub(crate) fn explicit_references_help(allowed: &str) -> String {
    references_help(allowed)
}

/// A Python `bool | int | str` setting value as help text shows it.
fn setting_text(value: &Bound<'_, PyAny>) -> PyResult<SettingText> {
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

/// Render `<purpose>, set this in <file>:` followed by the exact TOML section and key.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn setting_help<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "setting_help",
        options,
        (&["purpose", "file_name", "section", "key", "value"], &[]),
    )?;
    let text = |name: &str| -> PyResult<String> { arguments.value(name)?.extract() };
    let (file_name, section, key): (String, String, String) =
        (text("file_name")?, text("section")?, text("key")?);
    Ok(setting_help_text(
        &text("purpose")?,
        SettingSite {
            file_name: &file_name,
            section: &section,
            key: &key,
        },
        &setting_text(&arguments.value("value")?)?,
    ))
}

/// Render `<purpose>, <target>:` followed by the exact lines to write, one per line.
#[pyfunction]
#[pyo3(signature = (*, purpose, target, lines))]
fn snippet_help(purpose: &str, target: &str, lines: Vec<String>) -> String {
    snippet_help_text(purpose, target, &lines)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(setting_help, module)?)?;
    module.add_function(wrap_pyfunction!(snippet_help, module)?)?;
    module.add_function(wrap_pyfunction!(sql_analysis_off_guidance, module)?)?;
    module.add_function(wrap_pyfunction!(microbatch_guidance, module)?)?;
    module.add_function(wrap_pyfunction!(explicit_references_help, module)?)?;
    Ok(())
}
