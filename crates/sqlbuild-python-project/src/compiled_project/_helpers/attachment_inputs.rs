//! The resource rows and settings the compile attaches into the project, and the installed runs.

use crate::compiled_project::models::AuditFileRow;
use crate::compiled_project::models::AuditSettings;
use crate::compiled_project::models::DiagnosticSettings;
use crate::compiled_project::models::FunctionRow;
use crate::compiled_project::models::FunctionSettings;
use crate::compiled_project::models::ResourceAttachment;
use crate::compiled_project::models::ScenarioRow;
use crate::compiled_project::models::SqlTestSettings;
use crate::compiled_project::models::TestFileRow;
use crate::compiled_project::types::AttachedSqlTests;
use crate::compiled_project::types::ResourceDiagnostics;
use crate::compiled_project::types::SourceRow;
use crate::compiled_project::types::SourceSettings;
use std::sync::OnceLock;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::compiled_project::models::NativeCompiledProject;
use crate::resource_views::models::NativeAuditInput;
use crate::resource_views::models::NativeFunctionInput;
use crate::resource_views::models::NativeSourceInput;

const ATTACHMENT_UNINSTALLED: &str = "the native module registered no resource attachment";

pub(crate) static INSTALLED: OnceLock<ResourceAttachment> = OnceLock::new();

/// Attaches the project's sources, given their rows and settings.
pub(crate) type AttachSources = fn(
    &Bound<'_, NativeCompiledProject>,
    Vec<SourceRow>,
    SourceSettings,
) -> PyResult<Vec<NativeSourceInput>>;

/// Attaches the project's SQL, then Python, functions.
pub(crate) type AttachFunctions = fn(
    &Bound<'_, NativeCompiledProject>,
    (Vec<FunctionRow>, Vec<FunctionRow>),
    FunctionSettings,
) -> PyResult<Vec<NativeFunctionInput>>;

/// Reads the project's resource diagnostics.
pub(crate) type ResourceDiagnosticsRun = for<'py> fn(
    &NativeCompiledProject,
    Python<'py>,
    &DiagnosticSettings,
) -> PyResult<ResourceDiagnostics<'py>>;

/// Attaches the project's SQL test files, then its scenarios.
pub(crate) type AttachSqlTests = fn(
    &Bound<'_, NativeCompiledProject>,
    Vec<TestFileRow>,
    Vec<ScenarioRow>,
    SqlTestSettings,
) -> PyResult<AttachedSqlTests>;

/// Attaches the project's audits; returns their views and the built-in shadow warnings.
pub(crate) type AttachAudits =
    for<'py> fn(
        &Bound<'py, NativeCompiledProject>,
        Vec<AuditFileRow>,
        AuditSettings,
    ) -> PyResult<(Vec<NativeAuditInput>, Vec<Bound<'py, PyAny>>)>;

/// The installed attachment runs.
pub(crate) fn resource_attachment() -> PyResult<&'static ResourceAttachment> {
    INSTALLED
        .get()
        .ok_or_else(|| PyRuntimeError::new_err(ATTACHMENT_UNINSTALLED))
}
