//! Render `<purpose>, set this in <file>:` followed by the exact TOML section and key.

use crate::discovery::_helpers::failures::guidance::setting_text;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_config::project::main::setting_help_text::setting_help_text;
use sqlbuild_config::project::models::SettingSite;
use sqlbuild_python_base::boundary::models::KeywordArguments;

/// Render `<purpose>, set this in <file>:` followed by the exact TOML section and key.
#[pyfunction]
#[pyo3(signature = (**options))]
pub fn setting_help<'py>(py: Python<'py>, options: Option<Bound<'py, PyDict>>) -> PyResult<String> {
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
