//! Interpolation sentinels, inline directive findings and fixture-null candidates over lint models.

use std::path::PathBuf;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};
use sqlbuild_rules::lint_text::errors::LintTextError;
use sqlbuild_rules::lint_text::main::format_sql_bodies::format_sql_bodies;
use sqlbuild_rules::lint_text::main::named_declaration_role_kind::named_declaration_role_kind as role_kind;
use sqlbuild_rules::lint_text::main::neutralize_interpolation::neutralize_interpolation;
use sqlbuild_rules::lint_text::main::reject_inline_suppressions::reject_inline_suppressions;
use sqlbuild_rules::lint_text::main::restore_interpolation::restore_interpolation;
use sqlbuild_rules::lint_text::models::{
    FormatFault, FormatFile, FormattedBodies, InterpolationSite, Neutralized, SuppressionFinding,
};

use crate::bindings::_helpers::boundary::panics::{compiler_guard, value_error};
use crate::bindings::_helpers::records::construct::{
    enum_from_value, enum_member, new_record_with,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::rules::lint_pipeline::{
    LintViolationRow, NATIVE_ENGINE, ViolationClasses, project_relative_path,
};
use crate::bindings::models::Positional;
use crate::bindings::types::CompilerDetach;

const FORMAT_UNSAFE_CODE: &str = "format-unsafe";
const FORMAT_UNSAFE_REMEDIATION: &str = "Rewrite the unsupported SQL shape or report a minimal formatter reproduction. No header, SQL body, or fixture changes were written to this file.";

fn site_tuples(neutralized: Neutralized) -> (String, Vec<Positional<InterpolationSite>>) {
    (neutralized.text, Positional::wrap_all(neutralized.sites))
}

#[pyfunction]
#[pyo3(signature = (body, dialect, for_formatting = false))]
fn neutralize_lint_interpolation(
    body: &str,
    dialect: &str,
    for_formatting: bool,
) -> PyResult<(String, Vec<Positional<InterpolationSite>>)> {
    compiler_guard(|| {
        neutralize_interpolation(body, dialect, for_formatting)
            .map(site_tuples)
            .map_err(value_error)
    })
}

/// Restored text, or `(False, reason)` when a sentinel is missing or repeated.
#[pyfunction]
fn restore_lint_interpolation(
    fixed: &str,
    sites: Vec<Positional<InterpolationSite>>,
) -> PyResult<(bool, String)> {
    let sites: Vec<InterpolationSite> = Positional::unwrap_all(sites);
    match compiler_guard(|| Ok(restore_interpolation(fixed, &sites)))? {
        Ok(restored) => Ok((true, restored)),
        Err(LintTextError::Restoration(message)) => Ok((false, message)),
        Err(error) => Err(value_error(error)),
    }
}

/// The named declaration kind of a project-relative path, as `DeclarationKind`.
#[pyfunction]
#[pyo3(signature = (*, relative_path))]
fn named_declaration_role_kind<'py>(
    relative_path: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let parts: Vec<String> = native_field(relative_path, "parts")?.extract()?;
    match role_kind(&parts) {
        Some(kind) => Ok(Some(enum_from_value(
            relative_path.py(),
            RecordId::DeclarationKind,
            kind,
        )?)),
        None => Ok(None),
    }
}

/// `violations` plus a rejection finding for every inline suppression directive, as lint models.
#[pyfunction]
pub(crate) fn reject_lint_suppressions<'py>(
    py: Python<'py>,
    violations: Vec<Bound<'py, PyAny>>,
    contents_by_path: Bound<'py, PyDict>,
    project_dir: PathBuf,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut paths: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut relative: Vec<String> = Vec::new();
    let mut contents: Vec<String> = Vec::new();
    for (path, text) in contents_by_path.iter() {
        relative.push(project_relative_path(
            &path.extract::<PathBuf>()?,
            &project_dir,
        ));
        paths.push(path);
        contents.push(text.extract()?);
    }
    let sources: Vec<(&str, &str)> = relative
        .iter()
        .zip(&contents)
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let findings: Vec<SuppressionFinding> =
        compiler_guard(|| reject_inline_suppressions(&sources).map_err(value_error))?;
    let classes: ViolationClasses<'py> = ViolationClasses::new(py)?;
    let mut retained: Vec<Bound<'py, PyAny>> = violations;
    for finding in findings {
        let row: LintViolationRow = LintViolationRow::suppression(finding);
        retained.push(classes.violation(&paths[row.file], &row)?);
    }
    Ok(retained)
}

/// The parts of `path` below `project_dir`, or `None` outside the project.
pub(crate) fn relative_parts(
    path: &std::path::Path,
    project_dir: &std::path::Path,
) -> Option<Vec<String>> {
    match path.strip_prefix(project_dir) {
        Ok(relative) => Some(
            relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect(),
        ),
        Err(_) => None,
    }
}

/// The `format-unsafe` fault of a body the formatter declined in the file at `file_path`.
pub(crate) fn unsafe_fault<'py>(
    file_path: &Bound<'py, PyAny>,
    fault: &FormatFault,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = file_path.py();
    let severity: Bound<'py, PyAny> = enum_member(py, RecordId::LintSeverity, "FAULT")?;
    new_record_with(
        py,
        RecordId::LintViolation,
        &[
            ("file_path", file_path.clone()),
            ("line", fault.line.into_pyobject(py)?.into_any()),
            ("column", 1_usize.into_pyobject(py)?.into_any()),
            ("code", PyString::new(py, FORMAT_UNSAFE_CODE).into_any()),
            (
                "message",
                PyString::new(
                    py,
                    &format!(
                        "File left unchanged because its SQL body could not be formatted: {}",
                        fault.reason
                    ),
                )
                .into_any(),
            ),
            ("severity", severity),
            ("engine", PyString::new(py, NATIVE_ENGINE).into_any()),
            (
                "remediation",
                PyString::new(py, FORMAT_UNSAFE_REMEDIATION).into_any(),
            ),
        ],
    )
}

/// Format the SQL bodies of `files`; return changed texts by path and declined-body faults.
#[pyfunction]
pub(crate) fn format_lint_sql_bodies<'py>(
    py: Python<'py>,
    files: Vec<(Bound<'py, PyAny>, String)>,
    project_dir: PathBuf,
    config: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyDict>, Vec<Bound<'py, PyAny>>)> {
    let inputs: Vec<FormatFile> = files
        .iter()
        .map(|(path, contents)| {
            Ok(FormatFile {
                relative_parts: relative_parts(&path.extract::<PathBuf>()?, &project_dir),
                contents: contents.clone(),
            })
        })
        .collect::<PyResult<_>>()?;
    let dialect: String = native_field(config, "dialect")?.extract()?;
    let line_width: usize = native_field(config, "line_width")?.extract()?;
    let result: FormattedBodies = py
        .compiler_detach(|| {
            format_sql_bodies(&inputs, &dialect, line_width).map_err(|error| error.to_string())
        })
        .map_err(value_error)?;
    let formatted: Bound<'py, PyDict> = PyDict::new(py);
    for (index, text) in result.formatted {
        formatted.set_item(&files[index].0, text)?;
    }
    let faults: Vec<Bound<'py, PyAny>> = result
        .faults
        .iter()
        .map(|item| unsafe_fault(&files[item.file].0, item))
        .collect::<PyResult<_>>()?;
    Ok((formatted, faults))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(neutralize_lint_interpolation, module)?)?;
    module.add_function(wrap_pyfunction!(restore_lint_interpolation, module)?)?;
    module.add_function(wrap_pyfunction!(format_lint_sql_bodies, module)?)?;
    module.add_function(wrap_pyfunction!(named_declaration_role_kind, module)?)?;
    module.add_function(wrap_pyfunction!(reject_lint_suppressions, module)?)
}
