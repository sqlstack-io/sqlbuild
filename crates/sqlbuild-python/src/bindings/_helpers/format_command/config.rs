//! The run's `LintConfig`, resolved from the project's configuration files.

use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_rules::lint_text::main::resolve_format_settings::resolve_format_settings;
use sqlbuild_rules::lint_text::models::FormatSettings;

use crate::bindings::_helpers::rules::rules_config::load_rules_config;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

/// Format settings resolved for one run.
pub(crate) struct FormatConfig {
    pub(crate) settings: FormatSettings,
}

/// Resolve the settings, validating the rules configuration when the project file exists.
pub(crate) fn format_config(
    py: Python<'_>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<FormatConfig> {
    let directory: std::path::PathBuf = project_dir.extract()?;
    let settings: FormatSettings =
        resolve_format_settings(Path::new(&directory)).map_err(PyValueError::new_err)?;
    if settings.has_project_config {
        let _ = load_rules_config(
            py,
            project_dir
                .call_method0("resolve")?
                .extract::<std::path::PathBuf>()?,
        )?;
    }
    Ok(FormatConfig { settings })
}

/// The Python `LintConfig` of `config`, linting as `dialect`.
pub(crate) fn lint_config<'py>(
    py: Python<'py>,
    config: &FormatConfig,
    dialect: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let settings: &FormatSettings = &config.settings;
    new_record_with(
        py,
        RecordId::LintConfig,
        &[
            (
                "line_width",
                settings.line_width.into_pyobject(py)?.into_any(),
            ),
            (
                "max_description_lines",
                settings.max_description_lines.into_pyobject(py)?.into_any(),
            ),
            ("dialect", PyString::new(py, dialect).into_any()),
            (
                "max_literal_length",
                settings.max_literal_length.into_pyobject(py)?.into_any(),
            ),
            (
                "max_ranking_order_by",
                settings.max_ranking_order_by.into_pyobject(py)?.into_any(),
            ),
        ],
    )
}
