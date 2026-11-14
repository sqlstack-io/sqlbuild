//! The resource rows and settings the compile attaches into the project, and the installed runs.

use std::collections::HashMap;
use std::sync::OnceLock;

use pyo3::FromPyObject;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::types::{PyDict, PyString};

use crate::bindings::_helpers::cache::sql_test_scan_store::SqlTestScanStore;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::audit_views::NativeAuditInput;
use crate::bindings::_helpers::compiled_views::function_views::NativeFunctionInput;
use crate::bindings::_helpers::compiled_views::source_views::NativeSourceInput;

const ATTACHMENT_UNINSTALLED: &str = "the native module registered no resource attachment";

static INSTALLED: OnceLock<ResourceAttachment> = OnceLock::new();

/// One discovered source: its file's path, the discovered file and the entry.
pub(crate) type SourceRow = (String, Py<PyAny>, Py<PyAny>);

/// `(default contract, whether expressions are syntax-checked, source kind)`.
pub(crate) type SourceSettings = (Option<String>, bool, Py<PyAny>);

positional_record! {
    extract;
    /// A function's paths, stem, discovered file, header values, body and SQL file contents.
    pub(crate) struct FunctionRow {
        pub(crate) file_path: String,
        pub(crate) relative_path: String,
        pub(crate) name: String,
        pub(crate) function_file: Py<PyAny>,
        pub(crate) header_values: Py<PyDict>,
        pub(crate) body: String,
        pub(crate) contents: Option<String>,
    }
}

positional_record! {
    extract;
    /// Adapter, validation, raw default and target namespaces, inheritance and resource kind.
    pub(crate) struct FunctionSettings {
        pub(crate) adapter_name: String,
        pub(crate) validate: bool,
        pub(crate) defaults: (Option<String>, Option<String>),
        pub(crate) target: Option<(Option<String>, Option<String>)>,
        pub(crate) inherit: bool,
        pub(crate) kind: Py<PyAny>,
    }
}

/// A test case's discovered case, name and parameter values in authored order.
pub(crate) type TestCaseRow = (Py<PyAny>, String, Vec<(String, Py<PyAny>)>);

positional_record! {
    extract;
    /// A test block, its name, SQL, mode, declared parameter names, cases and authored SQL span.
    pub(crate) struct TestBlockRow {
        pub(crate) block: Py<PyAny>,
        pub(crate) name: Option<String>,
        pub(crate) sql_body: String,
        pub(crate) mode: String,
        pub(crate) parameters: Vec<String>,
        pub(crate) cases: Vec<TestCaseRow>,
        pub(crate) sql_body_span: Option<(usize, usize)>,
    }
}

positional_record! {
    extract;
    /// A test file's paths, stem, discovered file, contents and blocks.
    pub(crate) struct TestFileRow {
        pub(crate) file_path: String,
        pub(crate) relative_path: String,
        pub(crate) stem: String,
        pub(crate) file: Py<PyAny>,
        pub(crate) contents: String,
        pub(crate) blocks: Vec<TestBlockRow>,
    }
}

/// What every SQL test and scenario of one compile resolves and records with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct SqlTestSettings {
    /// `CompileSqlTestInput`, its model and direct payloads, and `CompileSqlTestCte`.
    pub(crate) test_input: Py<PyAny>,
    pub(crate) model_payload: Py<PyAny>,
    pub(crate) direct_payload: Py<PyAny>,
    pub(crate) test_cte: Py<PyAny>,
    /// `CompileSqlScenarioInput` and `CompileSqlScenarioCte`.
    pub(crate) scenario_input: Py<PyAny>,
    pub(crate) scenario_cte: Py<PyAny>,
    /// `SqlTestMode` by value.
    pub(crate) modes: Py<PyDict>,
    /// The `ResourceKind`s of tests and scenarios, and the runtime `UsageKind`.
    pub(crate) test_kind: Py<PyAny>,
    pub(crate) scenario_kind: Py<PyAny>,
    pub(crate) runtime_usage: Py<PyAny>,
    /// The adapter's typed scalar renderer, returning `(text, None)` or `(None, error)`.
    pub(crate) render_parameter: Py<PyAny>,
    pub(crate) adapter_name: String,
    /// `SourceLocation`, for helper problems located natively.
    pub(crate) source_location: Py<PyAny>,
    /// The shared scan store with the scan algorithm and lexical rules keying it, if any.
    pub(crate) scan_store: Option<Py<SqlTestScanStore>>,
    pub(crate) scan_key: (String, String),
}

/// The attached SQL test inputs, then scenario inputs.
pub(crate) type AttachedSqlTests = (Vec<Py<PyAny>>, Vec<Py<PyAny>>);

positional_record! {
    extract;
    /// A scenario file's paths, discovered file, contents, SQL body and name.
    pub(crate) struct ScenarioRow {
        pub(crate) file_path: String,
        pub(crate) relative_path: String,
        pub(crate) scenario_file: Py<PyAny>,
        pub(crate) contents: String,
        pub(crate) sql_body: String,
        pub(crate) name: String,
    }
}

positional_record! {
    extract, into;
    /// An audit block, its name, SQL body, evidence SQL, measurement flag and header values.
    pub(crate) struct AuditBlockRow {
        pub(crate) block: Py<PyAny>,
        pub(crate) name: Option<String>,
        pub(crate) sql_body: String,
        pub(crate) evidence_sql: Option<String>,
        pub(crate) measurement: bool,
        pub(crate) header: Py<PyDict>,
    }
}

positional_record! {
    extract, into;
    /// An audit file's paths, stem, discovered file, generic flag, contents and blocks.
    pub(crate) struct AuditFileRow {
        pub(crate) file_path: String,
        pub(crate) relative_path: String,
        pub(crate) stem: String,
        pub(crate) file: Py<PyAny>,
        pub(crate) generic: bool,
        pub(crate) contents: String,
        pub(crate) blocks: Vec<AuditBlockRow>,
    }
}

/// What every audit in one compile resolves against, and the Python values it builds with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct AuditSettings {
    pub(crate) default_severity: Option<String>,
    pub(crate) default_run_scope: Option<String>,
    /// Built-in generic definitions as audit file rows, overridden by project definitions.
    pub(crate) builtins: Vec<AuditFileRow>,
    /// `DeclarationIdentity`, and the singular and generic audit `DeclarationKind`s.
    pub(crate) declaration_identity: Py<PyAny>,
    pub(crate) singular_kind: Py<PyAny>,
    pub(crate) audit_kind: Py<PyAny>,
    /// `ResourceKind` by value, for the resources singular audits read and attach to.
    pub(crate) resource_kinds: Py<PyDict>,
}

/// A loader's description, relative path and whether that path is a YAML file.
pub(crate) type LoaderRow = (Option<String>, String, bool);

positional_record! {
    extract;
    /// A Python resource's kind, name, description (a `str` that may hold surrogates), location, help.
    pub(crate) struct PythonResourceRow {
        pub(crate) kind: String,
        pub(crate) name: String,
        pub(crate) description: Option<Py<PyString>>,
        pub(crate) location: Py<PyAny>,
        pub(crate) help: String,
    }
}

positional_record! {
    extract;
    /// A scenario's name, relative path, contents and authored header description.
    pub(crate) struct ScenarioDescriptionRow {
        pub(crate) name: String,
        pub(crate) path: String,
        pub(crate) contents: String,
        pub(crate) raw: Py<PyAny>,
    }
}

positional_record! {
    extract;
    /// A SQL hook file's name, relative path, contents and description.
    pub(crate) struct HookDescriptionRow {
        pub(crate) name: String,
        pub(crate) path: String,
        pub(crate) contents: String,
        pub(crate) description: Option<String>,
    }
}

/// What the resource diagnostics read beyond the owner, and the Python values they build.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct DiagnosticSettings {
    /// `CompilerDiagnostic`, `SourceLocation`, `Path` and `CompileInputError`.
    pub(crate) compiler_diagnostic: Py<PyAny>,
    pub(crate) compile_error: Py<PyAny>,
    pub(crate) source_location: Py<PyAny>,
    pub(crate) path_class: Py<PyAny>,
    /// Every discovered model file, whose headers attach audit factories.
    pub(crate) model_files: Vec<Py<PyAny>>,
    /// Each discovered audit factory's name, relative path and line.
    pub(crate) audit_factories: Vec<(String, String, usize)>,
    /// Each scenario's name, relative path, contents and authored header description.
    pub(crate) scenarios: Vec<ScenarioDescriptionRow>,
    /// Each SQL hook file's name, relative path, contents and description.
    pub(crate) sql_hooks: Vec<HookDescriptionRow>,
    /// Each loader by name.
    pub(crate) loaders: HashMap<String, LoaderRow>,
    /// Python-defined resources: kind, name, description, location and help, read by Python.
    pub(crate) python_resources: Vec<PythonResourceRow>,
    /// The project's `max_description_lines` and `line_width`, which Python descriptions keep to.
    pub(crate) description_limit: (usize, i64),
}

/// The orphaned audit factory warnings, then the missing descriptions, then the cursor errors.
pub(crate) type ResourceDiagnostics<'py> = (
    Vec<Bound<'py, PyAny>>,
    Vec<Bound<'py, PyAny>>,
    Vec<Bound<'py, PyAny>>,
);

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

/// The compile's runs behind the project's attachment methods.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResourceAttachment {
    pub(crate) sources: AttachSources,
    pub(crate) functions: AttachFunctions,
    pub(crate) diagnostics: ResourceDiagnosticsRun,
    pub(crate) sql_tests: AttachSqlTests,
    pub(crate) audits: AttachAudits,
}

/// Install the compile's attachment runs once, at registration.
pub(crate) fn install(attachment: ResourceAttachment) {
    let _ = INSTALLED.get_or_init(|| attachment);
}

/// The installed attachment runs.
pub(crate) fn resource_attachment() -> PyResult<&'static ResourceAttachment> {
    INSTALLED
        .get()
        .ok_or_else(|| PyRuntimeError::new_err(ATTACHMENT_UNINSTALLED))
}
