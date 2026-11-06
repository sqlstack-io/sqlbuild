//! Offline compiles that feed and verify a refactoring.

use std::path::Path;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyString, PyTuple};

use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::compile_command::target_writer::{Prepared, planning_diagnostics};
use crate::bindings::_helpers::compile_driver::analysis::analyze_compile_project;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::{enum_member, new_record, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

const DEFAULT_ERROR_CODE: &str = "E001";

/// One offline compile's project facts, or `None`, and every error that stopped or followed it.
pub(crate) struct OfflineCompile<'py> {
    pub(crate) project: Bound<'py, PyAny>,
    pub(crate) errors: Vec<Bound<'py, PyAny>>,
}

fn matches_any_class(py: Python<'_>, error: &PyErr, classes: &[ErrorId]) -> PyResult<bool> {
    for class in classes {
        if error.value(py).is_instance(&error_class(py, *class)?)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A compile failure, an exception rather than a native record, as its one error diagnostic.
fn compile_error_diagnostic<'py>(py: Python<'py>, error: &PyErr) -> PyResult<Bound<'py, PyAny>> {
    let value: Bound<'py, PyAny> = error.value(py).clone().into_any();
    let code: Bound<'py, PyAny> = match value.getattr_opt("code")? {
        Some(code) if !code.is_none() => code.str()?.into_any(),
        _ => PyString::new(py, DEFAULT_ERROR_CODE).into_any(),
    };
    let message: Bound<'py, PyAny> = match value.getattr_opt("message")? {
        Some(message) if !message.is_none() => message.str()?.into_any(),
        _ => value.str()?.into_any(),
    };
    new_record_with(
        py,
        RecordId::CompilerDiagnostic,
        &[
            (
                "phase",
                enum_member(py, RecordId::DiagnosticPhase, "COMPILE")?,
            ),
            (
                "severity",
                enum_member(py, RecordId::DiagnosticSeverity, "ERROR")?,
            ),
            ("code", code),
            ("message", message),
        ],
    )
}

/// An exception's attribute, or `None` when it has none.
fn attribute<'py>(value: &Bound<'py, PyAny>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    Ok(value
        .getattr_opt(name)?
        .unwrap_or_else(|| value.py().None().into_bound(value.py())))
}

/// A SQL-test planning failure as the test-phase error diagnostic it reports.
fn planning_error_diagnostic<'py>(py: Python<'py>, error: &PyErr) -> PyResult<Bound<'py, PyAny>> {
    let value: Bound<'py, PyAny> = error.value(py).clone().into_any();
    let location: Bound<'py, PyAny> =
        if value.is_instance(&error_class(py, ErrorId::SqlTestReference)?)? {
            value
                .getattr_opt("location")?
                .unwrap_or_else(|| py.None().into_bound(py))
        } else {
            py.None().into_bound(py)
        };
    new_record_with(
        py,
        RecordId::CompilerDiagnostic,
        &[
            ("phase", enum_member(py, RecordId::DiagnosticPhase, "TEST")?),
            (
                "severity",
                enum_member(py, RecordId::DiagnosticSeverity, "ERROR")?,
            ),
            ("code", attribute(&value, "code")?),
            ("message", value.str()?.into_any()),
            ("help", attribute(&value, "help")?),
            ("location", location),
        ],
    )
}

fn error_diagnostics<'py>(diagnostics: Bound<'py, PyAny>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut errors: Vec<Bound<'py, PyAny>> = Vec::new();
    for diagnostic in diagnostics.try_iter()? {
        let diagnostic: Bound<'py, PyAny> = diagnostic?;
        if native_field(&diagnostic, "is_error")?.is_truthy()? {
            errors.push(diagnostic);
        }
    }
    Ok(errors)
}

fn analyzed<'py>(
    py: Python<'py>,
    project_dir: &Path,
    no_cache: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let selection: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::CompileAnalysisSelection,
        &[("no_cache", PyBool::new(py, no_cache).to_owned().into_any())],
    )?;
    analyze_compile_project(
        py,
        Some(keywords(
            py,
            &[
                ("project_dir", project_dir.into_pyobject(py)?.into_any()),
                (
                    "no_sql_validation",
                    PyBool::new(py, false).to_owned().into_any(),
                ),
                ("selected_target", py.None().into_bound(py)),
                (
                    "lineage_mode",
                    enum_member(py, RecordId::CompileLineageMode, "NONE")?,
                ),
                ("cli_vars", py.None().into_bound(py)),
                (
                    "profile_flags",
                    new_record(py, RecordId::CompileProfileFlags, (), None)?,
                ),
                ("analysis_selection", selection),
                ("status", py.None().into_bound(py)),
            ],
        )?),
    )
}

/// Compile a project the way `sqb compile` does and collect every error.
pub(crate) fn compile_for_refactor<'py>(
    py: Python<'py>,
    project_dir: &Path,
    no_cache: bool,
) -> PyResult<OfflineCompile<'py>> {
    let analysis: Bound<'py, PyAny> = match analyzed(py, project_dir, no_cache) {
        Ok(analysis) => analysis,
        Err(error)
            if error.is_instance_of::<PyValueError>(py)
                || matches_any_class(py, &error, &[ErrorId::Rules, ErrorId::SpecConfig])? =>
        {
            return Ok(OfflineCompile {
                project: py.None().into_bound(py),
                errors: vec![compile_error_diagnostic(py, &error)?],
            });
        }
        Err(error) => return Err(error),
    };
    let mut errors: Vec<Bound<'py, PyAny>> =
        error_diagnostics(native_field(&analysis, "diagnostics")?)?;
    let rule_phase: Bound<'py, PyAny> = enum_member(py, RecordId::DiagnosticPhase, "RULE")?;
    let mut only_rule_failures: bool = true;
    for error in &errors {
        only_rule_failures &= native_field(error, "phase")?.is(&rule_phase);
    }
    if only_rule_failures {
        let planned: PyResult<Bound<'py, PyTuple>> = planning_diagnostics(
            &project_dir.join("target").into_pyobject(py)?.into_any(),
            (
                &native_field(&analysis, "adapter")?,
                &native_field(&native_field(&analysis, "graph")?, "project")?,
            ),
            Prepared::default(),
        );
        match planned {
            Ok(diagnostics) => errors.extend(error_diagnostics(diagnostics.into_any())?),
            Err(error)
                if matches_any_class(
                    py,
                    &error,
                    &[ErrorId::CompileInput, ErrorId::PlannerInput],
                )? =>
            {
                errors.push(planning_error_diagnostic(py, &error)?);
            }
            Err(error) => return Err(error),
        }
    }
    let project: Bound<'py, PyAny> = new_record_with(
        py,
        RecordId::RefactorProject,
        &[
            ("project_dir", project_dir.into_pyobject(py)?.into_any()),
            ("graph", native_field(&analysis, "graph")?),
            ("discovered", native_field(&analysis, "discovered_inputs")?),
        ],
    )?;
    Ok(OfflineCompile { project, errors })
}
