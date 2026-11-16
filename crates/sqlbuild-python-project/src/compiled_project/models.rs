//! Public records of compiled project.

use crate::compiled_project::_helpers::attachment_inputs::AttachAudits;
use crate::compiled_project::_helpers::attachment_inputs::AttachFunctions;
use crate::compiled_project::_helpers::attachment_inputs::AttachSources;
use crate::compiled_project::_helpers::attachment_inputs::AttachSqlTests;
use crate::compiled_project::_helpers::attachment_inputs::ResourceDiagnosticsRun;
use crate::compiled_project::_helpers::attachment_inputs::resource_attachment;
use crate::compiled_project::_helpers::project::ATTACHMENT_LOOP_UNSET;
use crate::compiled_project::_helpers::project::INPUT_CLASSES_UNSET;
use crate::compiled_project::_helpers::project::NamedType;
use crate::compiled_project::_helpers::snapshot_execution::Completion;
use crate::compiled_project::_helpers::snapshot_execution::is_function;
use crate::compiled_project::_helpers::snapshot_execution::poisoned;
use crate::compiled_project::_helpers::snapshot_execution::severity;
use crate::compiled_project::_helpers::snapshot_worker::WorkerOwner;
use crate::compiled_project::_helpers::snapshot_worker::unavailable;
use crate::compiled_project::constants::NO_CHANGE_REASON;
use crate::compiled_project::constants::REPORT_ATTEMPTS;
use crate::compiled_project::constants::SKIP_ACTION;
use crate::compiled_project::constants::SQL_TEST_KEY_PREFIX;
use crate::compiled_project::constants::STORE_BUSY_STATUS;
use crate::compiled_project::types::AttachedSqlTests;
use crate::compiled_project::types::LoaderRow;
use crate::compiled_project::types::NodeKeys;
use crate::compiled_project::types::ResourceDiagnostics;
use crate::compiled_project::types::SourceRow;
use crate::compiled_project::types::SourceSettings;
use crate::compiled_project::types::TestCaseRow;
use crate::compiled_views::main::audit_views::audit_record::audit_record;
use crate::compiled_views::main::function_views::function_record::function_record;
use crate::compiled_views::main::seed_views::paired_seeds::paired_seeds;
use crate::compiled_views::models::CompiledViewClasses;
use crate::compiled_views::models::NativeAuditInput;
use crate::compiled_views::models::NativeCompiledAudit;
use crate::compiled_views::models::NativeCompiledFunction;
use crate::compiled_views::models::NativeCompiledModel;
use crate::compiled_views::models::NativeCompiledSeed;
use crate::compiled_views::models::NativeCompiledSource;
use crate::compiled_views::models::NativeFunctionInput;
use crate::compiled_views::models::NativeInputsView;
use crate::compiled_views::models::NativeProjectView;
use crate::compiled_views::models::NativeSeedInput;
use crate::compiled_views::models::NativeSourceInput;
use crate::compiled_views::models::PyProjectHost;
use crate::compiled_views::models::SeedNamespace;
use crate::compiled_views::types::AttachedAudit;
use crate::compiled_views::types::AttachedFunction;
use crate::compiled_views::types::AuditRecord;
use crate::compiled_views::types::FunctionRecord;
use crate::compiled_views::types::ModelInputRecord;
use crate::compiled_views::types::SeedRecord;
use crate::compiled_views::types::SeedViews;
use crate::compiled_views::types::SourceRecord;
use pyo3::FromPyObject;
use pyo3::exceptions::PyIndexError;
use pyo3::exceptions::PyKeyboardInterrupt;
use pyo3::exceptions::PyRuntimeError;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyAnyMethods;
use pyo3::prelude::PyRef;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyString;
use pyo3::types::PyStringMethods;
use sqlbuild_analysis::compiled_project::main::project_models::project_models;
use sqlbuild_analysis::compiled_project::main::record_model_analysis::record_model_analysis;
use sqlbuild_analysis::compiled_project::main::record_skipped_analysis::record_skipped_analysis;
use sqlbuild_analysis::compiled_project::models::CompiledModelFacts;
use sqlbuild_analysis::compiled_project::models::CompiledProjectFacts;
use sqlbuild_analysis::compiled_project::models::DynamicProofFact;
use sqlbuild_analysis::compiled_project::models::ModelAnalysisFact;
use sqlbuild_analysis::compiled_project::models::ModelSchemaFact;
use sqlbuild_core::models::ResourceKind;
use sqlbuild_core::redaction::main::redact_error_text::redact_error_text;
use sqlbuild_host::models::SqlTestOutcome;
use sqlbuild_host::types::ResultEnum;
use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::runs::models::NodeOutcome;
use sqlbuild_orchestration::runs::models::RunChange;
use sqlbuild_orchestration::runs::models::RunCommand;
use sqlbuild_orchestration::runs::models::RunExecution;
use sqlbuild_orchestration::runs::models::RunOutcome;
use sqlbuild_orchestration::snapshots::main::publish_remote_snapshot::publish_remote_snapshot;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use sqlbuild_orchestration::snapshots::models::SnapshotRecord;
use sqlbuild_orchestration::worker::errors::WorkerError;
use sqlbuild_orchestration::worker::main::connect_worker::connect_worker;
use sqlbuild_orchestration::worker::main::start_worker::start_worker;
use sqlbuild_project::models::CompiledProject;
use sqlbuild_project::types::InferredRow;
use sqlbuild_project::types::ObjectKeyRow;
use sqlbuild_project::types::SourceNamespace;
use sqlbuild_python_base::boundary::main::native_compiler_error::native_compiler_error;
use sqlbuild_python_base::cache::models::SqlTestScanStore;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_front::render::models::ConstantRendering;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use uuid::Uuid;

/// A function's paths, stem, discovered file, header values, body and SQL file contents.
#[derive(Debug)]
pub struct FunctionRow {
    pub file_path: String,
    pub relative_path: String,
    pub name: String,
    pub function_file: Py<PyAny>,
    pub header_values: Py<PyDict>,
    pub body: String,
    pub contents: Option<String>,
}

impl<'py> pyo3::FromPyObject<'py> for FunctionRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (file_path, relative_path, name, function_file, header_values, body, contents): (
            String,
            String,
            String,
            Py<PyAny>,
            Py<PyDict>,
            String,
            Option<String>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            file_path,
            relative_path,
            name,
            function_file,
            header_values,
            body,
            contents,
        })
    }
}

/// Adapter, validation, raw default and target namespaces, inheritance and resource kind.
#[derive(Debug)]
pub struct FunctionSettings {
    pub adapter_name: String,
    pub validate: bool,
    pub defaults: (Option<String>, Option<String>),
    pub target: Option<(Option<String>, Option<String>)>,
    pub inherit: bool,
    pub kind: Py<PyAny>,
}

impl<'py> pyo3::FromPyObject<'py> for FunctionSettings {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (adapter_name, validate, defaults, target, inherit, kind): (
            String,
            bool,
            (Option<String>, Option<String>),
            Option<(Option<String>, Option<String>)>,
            bool,
            Py<PyAny>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            adapter_name,
            validate,
            defaults,
            target,
            inherit,
            kind,
        })
    }
}

/// A test block, its name, SQL, mode, declared parameter names, cases and authored SQL span.
#[derive(Debug)]
pub struct TestBlockRow {
    pub block: Py<PyAny>,
    pub name: Option<String>,
    pub sql_body: String,
    pub mode: String,
    pub parameters: Vec<String>,
    pub cases: Vec<TestCaseRow>,
    pub sql_body_span: Option<(usize, usize)>,
}

impl<'py> pyo3::FromPyObject<'py> for TestBlockRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (block, name, sql_body, mode, parameters, cases, sql_body_span): (
            Py<PyAny>,
            Option<String>,
            String,
            String,
            Vec<String>,
            Vec<TestCaseRow>,
            Option<(usize, usize)>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            block,
            name,
            sql_body,
            mode,
            parameters,
            cases,
            sql_body_span,
        })
    }
}

/// A test file's paths, stem, discovered file, contents and blocks.
#[derive(Debug)]
pub struct TestFileRow {
    pub file_path: String,
    pub relative_path: String,
    pub stem: String,
    pub file: Py<PyAny>,
    pub contents: String,
    pub blocks: Vec<TestBlockRow>,
}

impl<'py> pyo3::FromPyObject<'py> for TestFileRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (file_path, relative_path, stem, file, contents, blocks): (
            String,
            String,
            String,
            Py<PyAny>,
            String,
            Vec<TestBlockRow>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            file_path,
            relative_path,
            stem,
            file,
            contents,
            blocks,
        })
    }
}

/// What every SQL test and scenario of one compile resolves and records with.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct SqlTestSettings {
    /// `CompileSqlTestInput`, its model and direct payloads, and `CompileSqlTestCte`.
    pub test_input: Py<PyAny>,
    pub model_payload: Py<PyAny>,
    pub direct_payload: Py<PyAny>,
    pub test_cte: Py<PyAny>,
    /// `CompileSqlScenarioInput` and `CompileSqlScenarioCte`.
    pub scenario_input: Py<PyAny>,
    pub scenario_cte: Py<PyAny>,
    /// `SqlTestMode` by value.
    pub modes: Py<PyDict>,
    /// The `ResourceKind`s of tests and scenarios, and the runtime `UsageKind`.
    pub test_kind: Py<PyAny>,
    pub scenario_kind: Py<PyAny>,
    pub runtime_usage: Py<PyAny>,
    /// The adapter's typed scalar renderer, returning `(text, None)` or `(None, error)`.
    pub render_parameter: Py<PyAny>,
    pub adapter_name: String,
    /// `SourceLocation`, for helper problems located natively.
    pub source_location: Py<PyAny>,
    /// The shared scan store with the scan algorithm and lexical rules keying it, if any.
    pub scan_store: Option<Py<SqlTestScanStore>>,
    pub scan_key: (String, String),
}

/// A scenario file's paths, discovered file, contents, SQL body and name.
#[derive(Debug)]
pub struct ScenarioRow {
    pub file_path: String,
    pub relative_path: String,
    pub scenario_file: Py<PyAny>,
    pub contents: String,
    pub sql_body: String,
    pub name: String,
}

impl<'py> pyo3::FromPyObject<'py> for ScenarioRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (file_path, relative_path, scenario_file, contents, sql_body, name): (
            String,
            String,
            Py<PyAny>,
            String,
            String,
            String,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            file_path,
            relative_path,
            scenario_file,
            contents,
            sql_body,
            name,
        })
    }
}

/// An audit block, its name, SQL body, evidence SQL, measurement flag and header values.
#[derive(Debug)]
pub struct AuditBlockRow {
    pub block: Py<PyAny>,
    pub name: Option<String>,
    pub sql_body: String,
    pub evidence_sql: Option<String>,
    pub measurement: bool,
    pub header: Py<PyDict>,
}

impl<'py> pyo3::FromPyObject<'py> for AuditBlockRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (block, name, sql_body, evidence_sql, measurement, header): (
            Py<PyAny>,
            Option<String>,
            String,
            Option<String>,
            bool,
            Py<PyDict>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            block,
            name,
            sql_body,
            evidence_sql,
            measurement,
            header,
        })
    }
}

impl<'py> pyo3::IntoPyObject<'py> for AuditBlockRow {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.block,
                self.name,
                self.sql_body,
                self.evidence_sql,
                self.measurement,
                self.header,
            ),
            py,
        )
    }
}

/// An audit file's paths, stem, discovered file, generic flag, contents and blocks.
#[derive(Debug)]
pub struct AuditFileRow {
    pub file_path: String,
    pub relative_path: String,
    pub stem: String,
    pub file: Py<PyAny>,
    pub generic: bool,
    pub contents: String,
    pub blocks: Vec<AuditBlockRow>,
}

impl<'py> pyo3::FromPyObject<'py> for AuditFileRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (file_path, relative_path, stem, file, generic, contents, blocks): (
            String,
            String,
            String,
            Py<PyAny>,
            bool,
            String,
            Vec<AuditBlockRow>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            file_path,
            relative_path,
            stem,
            file,
            generic,
            contents,
            blocks,
        })
    }
}

impl<'py> pyo3::IntoPyObject<'py> for AuditFileRow {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (
                self.file_path,
                self.relative_path,
                self.stem,
                self.file,
                self.generic,
                self.contents,
                self.blocks,
            ),
            py,
        )
    }
}

/// What every audit in one compile resolves against, and the Python values it builds with.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct AuditSettings {
    pub default_severity: Option<String>,
    pub default_run_scope: Option<String>,
    /// Built-in generic definitions as audit file rows, overridden by project definitions.
    pub builtins: Vec<AuditFileRow>,
    /// `DeclarationIdentity`, and the singular and generic audit `DeclarationKind`s.
    pub declaration_identity: Py<PyAny>,
    pub singular_kind: Py<PyAny>,
    pub audit_kind: Py<PyAny>,
    /// `ResourceKind` by value, for the resources singular audits read and attach to.
    pub resource_kinds: Py<PyDict>,
}

/// A Python resource's kind, name, description (a `str` that may hold surrogates), location, help.
#[derive(Debug)]
pub struct PythonResourceRow {
    pub kind: String,
    pub name: String,
    pub description: Option<Py<PyString>>,
    pub location: Py<PyAny>,
    pub help: String,
}

impl<'py> pyo3::FromPyObject<'py> for PythonResourceRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (kind, name, description, location, help): (
            String,
            String,
            Option<Py<PyString>>,
            Py<PyAny>,
            String,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            kind,
            name,
            description,
            location,
            help,
        })
    }
}

/// A scenario's name, relative path, contents and authored header description.
#[derive(Debug)]
pub struct ScenarioDescriptionRow {
    pub name: String,
    pub path: String,
    pub contents: String,
    pub raw: Py<PyAny>,
}

impl<'py> pyo3::FromPyObject<'py> for ScenarioDescriptionRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (name, path, contents, raw): (String, String, String, Py<PyAny>) =
            pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            name,
            path,
            contents,
            raw,
        })
    }
}

/// A SQL hook file's name, relative path, contents and description.
#[derive(Debug)]
pub struct HookDescriptionRow {
    pub name: String,
    pub path: String,
    pub contents: String,
    pub description: Option<String>,
}

impl<'py> pyo3::FromPyObject<'py> for HookDescriptionRow {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (name, path, contents, description): (String, String, String, Option<String>) =
            pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            name,
            path,
            contents,
            description,
        })
    }
}

/// What the resource diagnostics read beyond the owner, and the Python values they build.
#[derive(Debug, FromPyObject)]
#[pyo3(from_item_all)]
pub struct DiagnosticSettings {
    /// `CompilerDiagnostic`, `SourceLocation`, `Path` and `CompileInputError`.
    pub compiler_diagnostic: Py<PyAny>,
    pub compile_error: Py<PyAny>,
    pub source_location: Py<PyAny>,
    pub path_class: Py<PyAny>,
    /// Every discovered model file, whose headers attach audit factories.
    pub model_files: Vec<Py<PyAny>>,
    /// Each discovered audit factory's name, relative path and line.
    pub audit_factories: Vec<(String, String, usize)>,
    /// Each scenario's name, relative path, contents and authored header description.
    pub scenarios: Vec<ScenarioDescriptionRow>,
    /// Each SQL hook file's name, relative path, contents and description.
    pub sql_hooks: Vec<HookDescriptionRow>,
    /// Each loader by name.
    pub loaders: HashMap<String, LoaderRow>,
    /// Python-defined resources: kind, name, description, location and help, read by Python.
    pub python_resources: Vec<PythonResourceRow>,
    /// The project's `max_description_lines` and `line_width`, which Python descriptions keep to.
    pub description_limit: (usize, i64),
}

/// The compile's runs behind the project's attachment methods.
#[derive(Clone, Copy, Debug)]
pub struct ResourceAttachment {
    pub sources: AttachSources,
    pub functions: AttachFunctions,
    pub diagnostics: ResourceDiagnosticsRun,
    pub sql_tests: AttachSqlTests,
    pub audits: AttachAudits,
}

/// `(output proven, bare dynamic pivot, (family key, inferred type), failure, (family, type))`.
#[derive(Debug)]
pub struct ProofPy {
    pub output_proven: bool,
    pub bare_dynamic_pivot: bool,
    pub families: Vec<NamedType>,
    pub failure_reason: Option<String>,
    pub named_families: Vec<NamedType>,
}

impl<'py> pyo3::FromPyObject<'py> for ProofPy {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (output_proven, bare_dynamic_pivot, families, failure_reason, named_families): (
            bool,
            bool,
            Vec<NamedType>,
            Option<String>,
            Vec<NamedType>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            output_proven,
            bare_dynamic_pivot,
            families,
            failure_reason,
            named_families,
        })
    }
}

/// `(inferred, (name, type), nullable per column, has star, unchecked output columns)`.
#[derive(Debug)]
pub struct AnalysisPy {
    pub inferred: bool,
    pub inferred_columns: Vec<NamedType>,
    pub nullable_columns: Vec<bool>,
    pub fast_lineage_has_star: bool,
    pub unchecked: Vec<String>,
}

impl<'py> pyo3::FromPyObject<'py> for AnalysisPy {
    fn extract_bound(value: &pyo3::Bound<'py, pyo3::PyAny>) -> pyo3::PyResult<Self> {
        let (inferred, inferred_columns, nullable_columns, fast_lineage_has_star, unchecked): (
            bool,
            Vec<NamedType>,
            Vec<bool>,
            bool,
            Vec<String>,
        ) = pyo3::types::PyAnyMethods::extract(value)?;
        Ok(Self {
            inferred,
            inferred_columns,
            nullable_columns,
            fast_lineage_has_star,
            unchecked,
        })
    }
}

/// Name, relative path, members, value, value type and rendering of one declaration.
#[derive(Debug, FromPyObject)]
pub struct DeclarationPy<'py>(
    pub(crate) String,
    pub(crate) String,
    pub(crate) Vec<(String, Bound<'py, PyAny>)>,
    pub(crate) Bound<'py, PyAny>,
    pub(crate) Option<String>,
    pub(crate) Option<String>,
);

/// One compile's project, recorded by the stages that produce it, with its Python projections.
#[pyclass(module = "sqlbuild._native")]
#[derive(Debug, Default)]
pub struct NativeCompiledProject {
    pub(crate) project: CompiledProject<PyProjectHost>,
    /// The Python classes model input views project their attributes with.
    pub(crate) input_classes: Option<Py<PyAny>>,
    /// Shared immutable column projections, allocated only when Python consumers request them.
    pub(crate) inferred_column_projections: Mutex<HashMap<InferredRow, Py<PyAny>>>,
    /// The model loop and its constant rendering, which attach the project's other resources.
    pub(crate) attachment: Option<(Py<PyAny>, Py<ConstantRendering>)>,
}

impl NativeCompiledProject {
    /// The facts recorded so far, for native consumers such as the rules request.
    pub fn facts(&self) -> &CompiledProjectFacts {
        &self.project.facts
    }

    /// A project holding only `facts`, which analyses can be recorded into apart from the compile's.
    pub fn with_facts(facts: CompiledProjectFacts) -> Self {
        Self {
            project: CompiledProject::with_facts(facts),
            ..Self::default()
        }
    }

    /// Retain facts a producer finished natively.
    pub fn retain(&mut self, facts: CompiledModelFacts) {
        self.project.retain(facts);
    }

    /// Retain a finished model's input record; return its index for the model's view.
    pub fn retain_input(&mut self, record: ModelInputRecord) -> usize {
        self.project.retain_input(record)
    }

    /// Keep the classes model input views project their attributes with.
    pub fn set_input_classes(&mut self, classes: Py<PyAny>) {
        self.input_classes = Some(classes);
    }

    /// Count one lineage graph a project view built.
    pub fn record_graph_build(&mut self) {
        self.project.graph_builds += 1;
    }

    /// The kept model loop and its constant rendering.
    pub fn attachment_loop(&self, py: Python<'_>) -> PyResult<(Py<PyAny>, Py<ConstantRendering>)> {
        match &self.attachment {
            Some((model_loop, host)) => Ok((model_loop.clone_ref(py), host.clone_ref(py))),
            None => Err(PyRuntimeError::new_err(ATTACHMENT_LOOP_UNSET)),
        }
    }

    /// Keep the model loop and its constant rendering for attaching the project's other resources.
    pub fn keep_attachment_loop(&mut self, model_loop: Py<PyAny>, host: Py<ConstantRendering>) {
        self.attachment = Some((model_loop, host));
    }

    /// Retain attached functions; return the index of the first.
    pub fn extend_functions(&mut self, attached: Vec<AttachedFunction>) -> usize {
        let first: usize = self.project.functions.len();
        self.project
            .functions
            .extend(attached.into_iter().map(function_record));
        first
    }

    /// Retain attached audits; return the index of the first.
    pub fn extend_audits(&mut self, attached: Vec<AttachedAudit>) -> usize {
        let first: usize = self.project.audits.len();
        self.project
            .audits
            .extend(attached.into_iter().map(audit_record));
        first
    }

    /// Every attached audit, in attachment order.
    pub fn audits(&self) -> &[AuditRecord] {
        self.project.audits()
    }

    /// Every attached function, in attachment order.
    pub fn functions(&self) -> &[FunctionRecord] {
        self.project.functions()
    }

    /// Every attached source, in attachment order.
    pub fn sources(&self) -> &[SourceRecord] {
        self.project.sources()
    }

    /// Keep one attached source.
    pub fn retain_source(&mut self, source: SourceRecord) {
        self.project.sources.push(source);
    }

    /// Every attached seed, in attachment order.
    pub fn seeds(&self) -> &[SeedRecord] {
        self.project.seeds()
    }

    /// The schema fact recorded for an input record's model.
    pub fn input_schema_fact(&self, record: &ModelInputRecord) -> Option<&ModelSchemaFact> {
        self.project.input_schema_fact(record)
    }

    /// Every model's input record, in loop order.
    pub fn inputs(&self) -> &[ModelInputRecord] {
        &self.project.inputs
    }

    /// The input record at `index`.
    pub fn input(&self, index: usize) -> PyResult<&ModelInputRecord> {
        self.project
            .input(index)
            .ok_or_else(|| PyIndexError::new_err(index))
    }

    /// The input record at `index`, for recording what later stages finish.
    pub fn input_mut(&mut self, index: usize) -> PyResult<&mut ModelInputRecord> {
        self.project
            .inputs
            .get_mut(index)
            .ok_or_else(|| PyIndexError::new_err(index))
    }

    /// Record that the native parser skipped the model at `index`: it is not analysed.
    pub fn mark_analysis_skipped(&mut self, index: usize) -> PyResult<()> {
        let record: &mut ModelInputRecord = self.input_mut(index)?;
        record.sql_validation_enabled = false;
        let name: String = record.name.clone();
        record_skipped_analysis(&mut self.project.facts, &name);
        Ok(())
    }

    /// The classes model input views project their attributes with.
    pub fn input_classes<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.input_classes
            .as_ref()
            .map(|classes| classes.bind(py).clone())
            .ok_or_else(|| PyIndexError::new_err(INPUT_CLASSES_UNSET))
    }

    pub fn inferred_column<'py>(
        &self,
        py: Python<'py>,
        classes: &CompiledViewClasses<'py>,
        row: &InferredRow,
    ) -> PyResult<Bound<'py, PyAny>> {
        let (name, column_type, nullability) = row;
        if let Some(column) = self
            .inferred_column_projections
            .lock()
            .map_err(|error| native_compiler_error(error.to_string()))?
            .get(row)
        {
            return Ok(column.bind(py).clone());
        }
        let fields = PyDict::new(py);
        fields.set_item("name", name)?;
        fields.set_item("type", column_type.as_deref())?;
        fields.set_item("nullability", classes.nullability.call1((nullability,))?)?;
        let column = classes.inferred_column.call((), Some(&fields))?;
        self.inferred_column_projections
            .lock()
            .map_err(|error| native_compiler_error(error.to_string()))?
            .insert(row.clone(), column.clone().unbind());
        Ok(column)
    }
}

#[pymethods]
impl NativeCompiledProject {
    #[new]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Attach a recorded model's typed analysis; `False` when that model was never recorded.
    pub fn record_model_analysis(
        &mut self,
        name: &str,
        columns: AnalysisPy,
        dynamic_proof: Option<ProofPy>,
    ) -> bool {
        let AnalysisPy {
            inferred,
            inferred_columns,
            nullable_columns,
            fast_lineage_has_star,
            unchecked,
        } = columns;
        let analysis: ModelAnalysisFact = ModelAnalysisFact {
            inferred_columns,
            dynamic_proof: dynamic_proof.map(
                |ProofPy {
                     output_proven,
                     bare_dynamic_pivot,
                     families,
                     failure_reason,
                     named_families,
                 }| DynamicProofFact {
                    output_proven,
                    bare_dynamic_pivot,
                    families,
                    failure_reason,
                    named_families,
                },
            ),
            inferred,
            nullable_columns,
            fast_lineage_has_star,
            unchecked_output_columns: unchecked,
        };
        record_model_analysis(&mut self.project.facts, name, analysis)
    }

    /// Record each assembled native model view's final typed analysis; return how many.
    pub fn record_view_analyses(
        slf: &Bound<'_, Self>,
        views: Vec<PyRef<'_, NativeCompiledModel>>,
    ) -> PyResult<usize> {
        let py: Python<'_> = slf.py();
        let mut facts: Vec<(String, ModelAnalysisFact)> = Vec::with_capacity(views.len());
        for view in &views {
            facts.push(view.analysis_fact(py)?);
        }
        let mut owner = slf.borrow_mut();
        let mut recorded: usize = 0;
        for (name, analysis) in facts {
            recorded += usize::from(record_model_analysis(
                &mut owner.project.facts,
                &name,
                analysis,
            ));
        }
        Ok(recorded)
    }

    /// The compile inputs view of this owner's model inputs with these input fields.
    pub fn inputs_view(
        slf: Bound<'_, Self>,
        fields: Bound<'_, PyDict>,
    ) -> PyResult<NativeInputsView> {
        NativeInputsView::new(slf.unbind(), &fields)
    }

    /// The compiled project view of this owner's models with these project fields.
    pub fn project_view(
        slf: Bound<'_, Self>,
        fields: Bound<'_, PyDict>,
    ) -> PyResult<NativeProjectView> {
        NativeProjectView::new(slf.unbind(), &fields)
    }

    /// Pair seed declarations with their CSV files and keep them; return each seed's view.
    pub fn attach_seeds(
        slf: &Bound<'_, Self>,
        declarations: Vec<(Py<PyAny>, Py<PyAny>)>,
        seed_files: Vec<Py<PyAny>>,
        compile_error: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<NativeSeedInput>> {
        let seeds: Vec<SeedRecord> = paired_seeds(declarations, &seed_files, compile_error)?;
        let count: usize = seeds.len();
        slf.borrow_mut().project.seeds = seeds;
        Ok((0..count)
            .map(|index| NativeSeedInput::new(slf.clone().unbind(), index))
            .collect())
    }

    /// Keep the destinations native project assembly resolved, in seed order.
    pub(crate) fn record_seed_namespaces(
        &mut self,
        namespaces: Vec<SeedNamespace>,
    ) -> PyResult<()> {
        for (index, namespace) in namespaces.into_iter().enumerate() {
            self.project
                .seeds
                .get_mut(index)
                .ok_or_else(|| PyIndexError::new_err(index))?
                .set_namespace(namespace);
        }
        Ok(())
    }

    /// Attach every discovered source in order with the kept loop; return each source's view.
    pub fn attach_sources(
        slf: &Bound<'_, Self>,
        rows: Vec<SourceRow>,
        settings: SourceSettings,
    ) -> PyResult<Vec<NativeSourceInput>> {
        (resource_attachment()?.sources)(slf, rows, settings)
    }

    /// Attach every SQL, then Python, function with the kept loop; return each one's view.
    pub fn attach_functions(
        slf: &Bound<'_, Self>,
        rows: (Vec<FunctionRow>, Vec<FunctionRow>),
        settings: FunctionSettings,
    ) -> PyResult<Vec<NativeFunctionInput>> {
        (resource_attachment()?.functions)(slf, rows, settings)
    }

    /// The orphaned audit factory warnings, missing descriptions and cursor-input errors.
    pub(crate) fn resource_diagnostics<'py>(
        &self,
        py: Python<'py>,
        settings: DiagnosticSettings,
    ) -> PyResult<ResourceDiagnostics<'py>> {
        (resource_attachment()?.diagnostics)(self, py, &settings)
    }

    /// Attach every SQL test case, then every scenario, with the kept loop.
    pub fn attach_sql_tests(
        slf: &Bound<'_, Self>,
        files: Vec<TestFileRow>,
        scenarios: Vec<ScenarioRow>,
        settings: SqlTestSettings,
    ) -> PyResult<AttachedSqlTests> {
        (resource_attachment()?.sql_tests)(slf, files, scenarios, settings)
    }

    /// Attach singular, then resource-attached, audits; return views and override warnings.
    pub fn attach_audits<'py>(
        slf: &Bound<'py, Self>,
        files: Vec<AuditFileRow>,
        settings: AuditSettings,
    ) -> PyResult<(Vec<NativeAuditInput>, Vec<Bound<'py, PyAny>>)> {
        (resource_attachment()?.audits)(slf, files, settings)
    }
    /// Keep the scope dependency keys native project assembly resolved, in audit order.
    pub(crate) fn record_audit_scope_deps(&mut self, deps: Vec<Vec<ObjectKeyRow>>) -> PyResult<()> {
        for (index, keys) in deps.into_iter().enumerate() {
            self.project
                .audits
                .get_mut(index)
                .ok_or_else(|| PyIndexError::new_err(index))?
                .set_scope_deps(keys);
        }
        Ok(())
    }

    /// The assembled audit at `index`.
    pub(crate) fn compiled_audit(
        slf: &Bound<'_, Self>,
        index: usize,
    ) -> PyResult<NativeCompiledAudit> {
        if index >= slf.borrow().project.audits.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledAudit::new(slf.clone().unbind(), index))
    }

    /// Keep the dependency keys native project assembly resolved, in function order.
    pub(crate) fn record_function_deps(&mut self, deps: Vec<Vec<ObjectKeyRow>>) -> PyResult<()> {
        for (index, keys) in deps.into_iter().enumerate() {
            self.project
                .functions
                .get_mut(index)
                .ok_or_else(|| PyIndexError::new_err(index))?
                .set_deps(keys);
        }
        Ok(())
    }

    /// The assembled function at `index`.
    pub(crate) fn compiled_function(
        slf: &Bound<'_, Self>,
        index: usize,
    ) -> PyResult<NativeCompiledFunction> {
        if index >= slf.borrow().project.functions.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledFunction::new(slf.clone().unbind(), index))
    }

    /// Keep the namespaces native project assembly resolved, in source order.
    pub(crate) fn record_source_namespaces(
        &mut self,
        namespaces: Vec<SourceNamespace>,
    ) -> PyResult<()> {
        for (index, namespace) in namespaces.into_iter().enumerate() {
            self.project
                .sources
                .get_mut(index)
                .ok_or_else(|| PyIndexError::new_err(index))?
                .set_namespace(namespace);
        }
        Ok(())
    }

    /// The assembled source at `index`.
    pub(crate) fn compiled_source(
        slf: &Bound<'_, Self>,
        index: usize,
    ) -> PyResult<NativeCompiledSource> {
        if index >= slf.borrow().project.sources.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledSource::new(slf.clone().unbind(), index))
    }

    /// The assembled seed at `index`.
    pub(crate) fn compiled_seed(
        slf: &Bound<'_, Self>,
        index: usize,
    ) -> PyResult<NativeCompiledSeed> {
        if index >= slf.borrow().project.seeds.len() {
            return Err(PyIndexError::new_err(index));
        }
        Ok(NativeCompiledSeed::new(slf.clone().unbind(), index))
    }

    /// How many lineage graphs this owner's project views built.
    pub(crate) fn graph_builds(&self) -> usize {
        self.project.graph_builds
    }

    /// Whether both projects retained the same facts.
    pub(crate) fn __eq__(&self, other: &Self) -> bool {
        self.project.facts == other.project.facts
    }

    /// The recorded models' names, in production order.
    pub(crate) fn model_names(&self) -> Vec<String> {
        project_models(&self.project.facts)
            .iter()
            .map(|model| model.name.clone())
            .collect()
    }
}

/// A reference as the tuple Python reads: kind, name, package and call argument count.
#[derive(Debug)]
pub struct ReferenceTuple {
    pub kind: String,
    pub name: String,
    pub package: Option<String>,
    pub call_argument_count: Option<usize>,
}

impl<'py> pyo3::IntoPyObject<'py> for ReferenceTuple {
    type Target = pyo3::types::PyTuple;
    type Output = pyo3::Bound<'py, pyo3::types::PyTuple>;
    type Error = pyo3::PyErr;

    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {
        pyo3::IntoPyObject::into_pyobject(
            (self.kind, self.name, self.package, self.call_argument_count),
            py,
        )
    }
}

/// A data-only publication; this object never contains adapters, connections or user callables.
#[pyclass(frozen, module = "sqlbuild._native")]
#[derive(Debug)]
pub struct NativeOrchestrationSnapshot {
    pub input: SnapshotInput,
}

impl NativeOrchestrationSnapshot {
    /// Native callers retain transport errors so they can report an uncertain acknowledgement.
    pub fn publication(
        &self,
        py: Python<'_>,
        server_url: &str,
        bearer: &str,
    ) -> PyResult<Result<SnapshotRecord, ClientError>> {
        py.detach(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let result = runtime.block_on(publish_remote_snapshot(server_url, bearer, &self.input));
            runtime.shutdown_timeout(std::time::Duration::from_secs(1));
            Ok(result)
        })
    }
}

#[pymethods]
impl NativeOrchestrationSnapshot {
    /// Serialize only at the public boundary; native transports can use the retained input.
    pub(crate) fn to_json(&self, py: Python<'_>) -> PyResult<String> {
        py.detach(|| serde_json::to_string(&self.input))
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    /// Publish retained data without projecting it through Python, releasing the GIL during IO.
    pub(crate) fn publish(
        &self,
        py: Python<'_>,
        server_url: &str,
        bearer: &str,
    ) -> PyResult<String> {
        let record = self
            .publication(py, server_url, bearer)?
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        py.detach(|| {
            serde_json::to_string(&record)
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Retain the verified claim and its independent native heartbeat in the worker interpreter.
    pub(crate) fn start_worker(
        &self,
        py: Python<'_>,
        server_url: &str,
        bearer: &str,
        identity: (String, String),
    ) -> PyResult<NativeOrchestrationWorker> {
        NativeOrchestrationWorker::start(self, py, (server_url, bearer), identity)
    }
}

/// This transient owner records only acknowledged outcomes; durable lifecycle remains on the API.
#[derive(Debug)]
pub struct WorkerBuild {
    pub worker: Py<NativeOrchestrationWorker>,
    pub(crate) execution: RunExecution,
    pub(crate) completed: Mutex<BTreeMap<String, NodeOutcome>>,
    pub(crate) checks: Mutex<BTreeMap<String, Option<NodeOutcome>>>,
    /// The snapshot's audits and SQL tests, each with the node keys it checks.
    pub(crate) attached: BTreeMap<String, Vec<String>>,
    /// Each SQL test with every node a failure of it blocks in a build: its expected, assertion and reference models and the functions it checks.
    pub(crate) blocked: BTreeMap<String, Vec<String>>,
    /// How many of each function's planned tests have still to report; a function is built before its tests run.
    pub(crate) untested: Mutex<BTreeMap<String, usize>>,
    /// A built function's result, held until its tests report so a failing one can fail it instead.
    pub(crate) held: Mutex<BTreeMap<String, Completion>>,
    /// Each snapshot node's direct upstream node keys, for naming what a skipped node waited on.
    pub(crate) upstream: BTreeMap<String, Vec<String>>,
    /// The failed node each finished node traces back to: itself when it failed, its cause when skipped.
    pub(crate) causes: Mutex<BTreeMap<String, String>>,
    /// The worst attached outcome of each node, for commands that only run tests or audits.
    pub(crate) checked: Mutex<BTreeMap<String, NodeOutcome>>,
    /// Whether the command returned, having run every check it planned, rather than being refused or raising.
    pub(crate) returned: AtomicBool,
    /// Why the worker refused the run before executing anything, reported as the run's status reason.
    pub(crate) refusal: Mutex<Option<String>>,
}

impl WorkerBuild {
    pub fn new(
        worker: &Bound<'_, NativeOrchestrationWorker>,
        (attached, blocked, upstream): (NodeKeys, NodeKeys, NodeKeys),
    ) -> PyResult<Arc<Self>> {
        let state = Arc::new(Self {
            execution: worker.get().execution(worker.py())?,
            worker: worker.clone().unbind(),
            completed: Mutex::default(),
            checks: Mutex::default(),
            attached,
            blocked,
            untested: Mutex::default(),
            held: Mutex::default(),
            upstream,
            causes: Mutex::default(),
            checked: Mutex::default(),
            returned: AtomicBool::new(false),
            refusal: Mutex::default(),
        });
        for key in state.execution.skipped_nodes.keys() {
            state.complete(worker.py(), key, (NodeOutcome::Skipped, None, (None, None)))?;
        }
        Ok(state)
    }

    pub fn command(&self) -> RunCommand {
        self.execution.claim.request.command
    }

    /// The server run this build executes.
    pub fn run_id(&self) -> uuid::Uuid {
        self.execution.claim.request.run_id
    }

    /// Test and audit runs execute only attached checks, so their nodes take the checks' outcome.
    pub(crate) fn checks_only(&self) -> bool {
        matches!(self.command(), RunCommand::Test | RunCommand::Audit)
    }

    /// Record that the command returned instead of being refused or raising.
    pub fn command_returned(&self) {
        self.returned.store(true, Ordering::SeqCst);
    }

    pub fn has_work(&self) -> bool {
        self.execution
            .claim
            .request
            .selection
            .iter()
            .any(|key| !self.execution.skipped_nodes.contains_key(key))
    }

    /// Record why the run is refused; the terminal report carries it as the run's status reason.
    pub fn refuse(&self, reason: &str) -> PyResult<()> {
        *self.refusal.lock().map_err(|_| poisoned())? = Some(reason.to_owned());
        Ok(())
    }

    pub fn check_cancelled(&self, py: Python<'_>) -> PyResult<()> {
        if self.worker.bind(py).get().cancel_requested(py) {
            return Err(PyKeyboardInterrupt::new_err("run cancellation requested"));
        }
        Ok(())
    }

    /// Reject writers outside the frozen write set before any executor connection.
    pub fn enforce_writers(&self, py: Python<'_>, keys: &[String]) -> Result<(), pyo3::PyErr> {
        self.check_cancelled(py)?;
        match keys
            .iter()
            .find(|key| !self.execution.claim.request.write_set.contains(*key))
        {
            Some(key) => Err(PyValueError::new_err(format!(
                "planned writer {key} is outside the frozen run write set"
            ))),
            None => Ok(()),
        }
    }

    /// Reject an implicitly expanded write set before any executor connection or schema change.
    pub fn enforce_plan_writers(&self, pipeline: &Bound<'_, PyAny>) -> Result<(), pyo3::PyErr> {
        self.check_cancelled(pipeline.py())?;
        let plan = field(pipeline, "plan_output")?;
        self.count_planned_tests(&plan)?;
        for collection in [
            "model_entries",
            "seed_entries",
            "function_entries",
            "source_load_entries",
        ] {
            for entry in field(&plan, collection)?.try_iter()? {
                let entry = entry?;
                let identity = field(&entry, "key")?;
                let kind = field(&identity, "resource_type")?
                    .str()?
                    .to_str()?
                    .to_owned();
                let key = format!("{kind}:{}", field(&identity, "name")?.str()?);
                if !self.execution.claim.request.write_set.contains(&key) {
                    return Err(PyValueError::new_err(format!(
                        "planned writer {key} is outside the frozen run write set"
                    )));
                }
                if kind == ResourceKind::Model.as_str()
                    && field(&entry, "action")?.str()?.to_str()? == SKIP_ACTION
                {
                    let outcome = if field(&entry, "reason")?.str()?.to_str()? == NO_CHANGE_REASON {
                        NodeOutcome::Success
                    } else {
                        NodeOutcome::Skipped
                    };
                    self.complete(pipeline.py(), &key, (outcome, None, (None, None)))?;
                }
            }
        }
        Ok(())
    }

    /// Count each function's tests this build plans, so a built function waits only for tests that will report.
    pub(crate) fn count_planned_tests(&self, plan: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut untested = self.untested.lock().map_err(|_| poisoned())?;
        for entry in field(plan, "test_entries")?.try_iter()? {
            let test = format!("{SQL_TEST_KEY_PREFIX}{}", field(&entry?, "name")?.str()?);
            for node in self.blocked.get(&test).into_iter().flatten() {
                if is_function(node) {
                    *untested.entry(node.clone()).or_default() += 1;
                }
            }
        }
        Ok(())
    }

    /// Nodes `build --changed` leaves unbuilt are up to date, as a planner `no_change` skip is.
    pub fn unchanged(&self, py: Python<'_>, keys: &[String]) -> PyResult<()> {
        for key in keys {
            self.complete(py, key, (NodeOutcome::Success, None, (None, None)))?;
        }
        Ok(())
    }

    /// Nodes `build --changed` holds back wait for the job that refreshes them, with why.
    pub fn held_back(&self, py: Python<'_>, nodes: &[(String, String)]) -> PyResult<()> {
        for (key, why) in nodes {
            self.complete(
                py,
                key,
                (
                    NodeOutcome::Skipped,
                    None,
                    (None, Some(format!("held back: {why}"))),
                ),
            )?;
        }
        Ok(())
    }

    /// A skip in a build names the failed node upstream of it; a test or audit run that raised skips what it never checked, with no cause.
    pub(crate) fn complete(
        &self,
        py: Python<'_>,
        key: &str,
        (outcome, rows_affected, (error_code, error_message)): (
            NodeOutcome,
            Option<i64>,
            (Option<String>, Option<String>),
        ),
    ) -> PyResult<()> {
        if !self.execution.claim.request.selection.contains(key) {
            return Ok(());
        }
        if self
            .completed
            .lock()
            .map_err(|_| poisoned())?
            .contains_key(key)
        {
            return Ok(());
        }
        let skipped_because_of = match outcome {
            NodeOutcome::Skipped => match self.execution.skipped_nodes.get(key) {
                Some(cause) => Some(cause.clone()),
                None if !self.checks_only() => self.failed_upstream(key)?,
                None => None,
            },
            _ => None,
        };
        self.worker.bind(py).get().change(
            py,
            &format!("node-finished/{key}"),
            RunChange::NodeFinished {
                node_key: key.into(),
                outcome,
                rows_affected,
                error_code,
                skipped_because_of: skipped_because_of.clone(),
                error_message: error_message.as_deref().map(redact_error_text),
            },
        )?;
        let cause = match outcome {
            NodeOutcome::Failed => Some(key.to_owned()),
            _ => skipped_because_of,
        };
        if let Some(cause) = cause {
            self.causes
                .lock()
                .map_err(|_| poisoned())?
                .insert(key.into(), cause);
        }
        self.completed
            .lock()
            .map_err(|_| poisoned())?
            .insert(key.into(), outcome);
        Ok(())
    }

    pub fn start(
        &self,
        py: Python<'_>,
        name: &str,
        resource_kind: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.check_cancelled(py)?;
        let kind = field(resource_kind, "value")?.extract::<String>()?;
        let kind = match kind.as_str() {
            "table" | "view" | "snapshot" | "custom" => "model",
            kind => kind,
        };
        self.start_key(py, &format!("{kind}:{name}"))
    }

    /// Report a selected node as started unless its outcome is final, such as a planner skip.
    pub fn start_key(&self, py: Python<'_>, key: &str) -> PyResult<()> {
        self.check_cancelled(py)?;
        if self.execution.claim.request.selection.contains(key)
            && !self
                .completed
                .lock()
                .map_err(|_| poisoned())?
                .contains_key(key)
        {
            self.worker.bind(py).get().change(
                py,
                &format!("node-started/{key}"),
                RunChange::NodeStarted {
                    node_key: key.into(),
                },
            )?;
        }
        Ok(())
    }

    /// Report a selected node's outcome when the command reports no executor result for it.
    pub fn finish_key(&self, py: Python<'_>, key: &str, outcome: NodeOutcome) -> PyResult<()> {
        self.complete(py, key, (outcome, None, (None, None)))
    }

    /// A test or audit the snapshot records has started; a selected SQL test reports as a node.
    pub fn start_attached(&self, py: Python<'_>, key: &str) -> PyResult<()> {
        self.check_cancelled(py)?;
        let Some(nodes) = self.attached.get(key) else {
            return Ok(());
        };
        if self.execution.claim.request.selection.contains(key) {
            return self.start_key(py, key);
        }
        if self
            .checks
            .lock()
            .map_err(|_| poisoned())?
            .contains_key(key)
        {
            return Ok(());
        }
        self.worker.bind(py).get().change(
            py,
            &format!("check-started/{key}"),
            RunChange::CheckStarted {
                node_key: key.into(),
            },
        )?;
        self.checks
            .lock()
            .map_err(|_| poisoned())?
            .insert(key.into(), None);
        if self.checks_only() {
            for node in nodes {
                self.start_key(py, node)?;
            }
        }
        Ok(())
    }

    /// A test or audit finished; a check the snapshot does not record is not reported.
    pub fn finish_attached(&self, py: Python<'_>, key: &str, outcome: NodeOutcome) -> PyResult<()> {
        let Some(nodes) = self.attached.get(key) else {
            return Ok(());
        };
        if outcome != NodeOutcome::Skipped {
            self.start_attached(py, key)?;
        }
        let outcome = if outcome == NodeOutcome::Failed
            && self.worker.bind(py).get().refresh_cancellation(py)?
        {
            NodeOutcome::Cancelled
        } else {
            outcome
        };
        if self.execution.claim.request.selection.contains(key) {
            self.complete(py, key, (outcome, None, (None, None)))?;
        } else {
            self.finish_check(py, key, outcome)?;
        }
        if outcome == NodeOutcome::Skipped {
            return Ok(());
        }
        let mut checked = self.checked.lock().map_err(|_| poisoned())?;
        for node in nodes {
            let worst = checked.get(node).map_or(outcome, |previous| {
                if severity(outcome) > severity(*previous) {
                    outcome
                } else {
                    *previous
                }
            });
            checked.insert(node.clone(), worst);
        }
        Ok(())
    }

    /// The failed node nearest upstream of `key` in this run, or the cause a skipped one carries.
    pub(crate) fn failed_upstream(&self, key: &str) -> PyResult<Option<String>> {
        let causes = self.causes.lock().map_err(|_| poisoned())?;
        let mut seen: BTreeSet<&str> = BTreeSet::from([key]);
        let mut pending: VecDeque<&str> = VecDeque::from([key]);
        while let Some(node) = pending.pop_front() {
            for upstream in self.upstream.get(node).into_iter().flatten() {
                if let Some(cause) = causes.get(upstream) {
                    return Ok(Some(cause.clone()));
                }
                if seen.insert(upstream) {
                    pending.push_back(upstream);
                }
            }
        }
        Ok(None)
    }

    /// A selected SQL test completes itself; a failing one, found by its compiled name such as `parent [case]`, fails the nodes it blocks; each test that reports releases a function waiting on its tests.
    pub(crate) fn finished_test(&self, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let py = result.py();
        let outcome: String = field(result, "outcome")?.str()?.extract()?;
        let passed = SqlTestOutcome::from_value(&outcome) == Some(SqlTestOutcome::Pass);
        let cancelled = !passed && self.worker.bind(py).get().refresh_cancellation(py)?;
        let name: String = field(result, "test_name")?.extract()?;
        let key = format!("{SQL_TEST_KEY_PREFIX}{name}");
        let error: Option<String> = match result.getattr_opt("error_code")? {
            Some(value) => value.extract()?,
            None => None,
        };
        let message: Option<String> = match result.getattr_opt("error_message")? {
            Some(value) => value.extract()?,
            None => None,
        };
        let node = match (passed, cancelled) {
            (true, _) => NodeOutcome::Success,
            (false, true) => NodeOutcome::Cancelled,
            (false, false) => NodeOutcome::Failed,
        };
        self.complete(py, &key, (node, None, (error.clone(), message.clone())))?;
        if !passed && !cancelled {
            for model in self.blocked.get(&key).into_iter().flatten() {
                self.complete(
                    py,
                    model,
                    (NodeOutcome::Failed, None, (error.clone(), message.clone())),
                )?;
            }
        }
        self.release_tested(py, &key)
    }

    /// Count `test` as reported for each function it checks, and publish a held function once none of its tests is left.
    pub(crate) fn release_tested(&self, py: Python<'_>, test: &str) -> PyResult<()> {
        let mut released: Vec<(String, Completion)> = Vec::new();
        {
            let mut untested = self.untested.lock().map_err(|_| poisoned())?;
            let mut held = self.held.lock().map_err(|_| poisoned())?;
            for node in self.blocked.get(test).into_iter().flatten() {
                let Some(remaining) = untested.get_mut(node) else {
                    continue;
                };
                *remaining = remaining.saturating_sub(1);
                if *remaining == 0
                    && let Some(completion) = held.remove(node)
                {
                    released.push((node.clone(), completion));
                }
            }
        }
        for (node, completion) in released {
            self.complete(py, &node, completion)?;
        }
        Ok(())
    }

    /// Publish a node's result, or hold a built function's while its tests have still to report.
    pub(crate) fn complete_or_hold(
        &self,
        py: Python<'_>,
        key: &str,
        completion: Completion,
    ) -> PyResult<()> {
        let waiting = !self.checks_only()
            && matches!(completion.0, NodeOutcome::Success | NodeOutcome::Warn)
            && self
                .untested
                .lock()
                .map_err(|_| poisoned())?
                .get(key)
                .is_some_and(|remaining| *remaining > 0);
        if waiting {
            self.held
                .lock()
                .map_err(|_| poisoned())?
                .insert(key.into(), completion);
            return Ok(());
        }
        self.complete(py, key, completion)
    }

    /// Existing native progress callbacks publish selected-node evidence without Python relays.
    pub fn completed(&self, result: &Bound<'_, PyAny>, key: &str) -> PyResult<()> {
        if key.starts_with(SQL_TEST_KEY_PREFIX) {
            return self.finished_test(result);
        }
        if self.execution.claim.request.selection.contains(key) {
            let status: String = field(result, "status")?.str()?.extract()?;
            let outcome = match status.as_str() {
                "success" => {
                    if result.hasattr("warning_messages")?
                        && field(result, "warning_messages")?.is_truthy()?
                    {
                        NodeOutcome::Warn
                    } else {
                        NodeOutcome::Success
                    }
                }
                "failed" => {
                    if self
                        .worker
                        .bind(result.py())
                        .get()
                        .refresh_cancellation(result.py())?
                    {
                        NodeOutcome::Cancelled
                    } else {
                        NodeOutcome::Failed
                    }
                }
                "skipped" => NodeOutcome::Skipped,
                _ => {
                    return Err(PyValueError::new_err(format!(
                        "unsupported worker node outcome {status}"
                    )));
                }
            };
            let rows = match result.getattr_opt("rows_affected")? {
                Some(value) => value.extract()?,
                None => None,
            };
            let error = match result.getattr_opt("error_code")? {
                Some(value) => value.extract()?,
                None => None,
            };
            let message = match result.getattr_opt("error_message")? {
                Some(value) => value.extract()?,
                None => None,
            };
            self.complete_or_hold(result.py(), key, (outcome, rows, (error, message)))?;
        }
        Ok(())
    }

    pub fn start_check(&self, py: Python<'_>, name: &str) -> PyResult<()> {
        self.check_cancelled(py)?;
        let key = format!("check:{name}");
        self.worker.bind(py).get().change(
            py,
            &format!("check-started/{key}"),
            RunChange::CheckStarted {
                node_key: key.clone(),
            },
        )?;
        self.checks
            .lock()
            .map_err(|_| poisoned())?
            .insert(key, None);
        Ok(())
    }

    /// A completion without our actual-execution start callback is a blocked check, not a failure.
    pub fn completed_check(&self, result: &Bound<'_, PyAny>) -> PyResult<()> {
        let name: String = field(result, "node_name")?.extract()?;
        let key = format!("check:{name}");
        let started = self
            .checks
            .lock()
            .map_err(|_| poisoned())?
            .contains_key(&key);
        let outcome = if !started {
            NodeOutcome::Skipped
        } else if field(result, "passed")?.is_truthy()? {
            NodeOutcome::Success
        } else if field(result, "warned")?.is_truthy()? {
            NodeOutcome::Warn
        } else if self
            .worker
            .bind(result.py())
            .get()
            .refresh_cancellation(result.py())?
        {
            NodeOutcome::Cancelled
        } else {
            NodeOutcome::Failed
        };
        self.finish_check(result.py(), &key, outcome)
    }

    pub(crate) fn finish_check(
        &self,
        py: Python<'_>,
        key: &str,
        outcome: NodeOutcome,
    ) -> PyResult<()> {
        self.worker.bind(py).get().change(
            py,
            &format!("check-finished/{key}"),
            RunChange::CheckFinished {
                node_key: key.into(),
                outcome,
            },
        )?;
        self.checks
            .lock()
            .map_err(|_| poisoned())?
            .insert(key.into(), Some(outcome));
        Ok(())
    }

    /// Never call unseen nodes successful unless the executor returned; a refused command proves only its failures.
    pub fn outcome(&self, py: Python<'_>, exit_code: i32) -> PyResult<RunOutcome> {
        let cancelled = self.worker.bind(py).get().refresh_cancellation(py)?;
        let checks_only = self.checks_only();
        if checks_only && !cancelled {
            let checked = self.checked.lock().map_err(|_| poisoned())?.clone();
            let returned = self.returned.load(Ordering::SeqCst);
            for key in &self.execution.claim.request.selection {
                let seen = checked.get(key).copied();
                let outcome = if returned {
                    Some(seen.unwrap_or(NodeOutcome::Success))
                } else {
                    seen.filter(|outcome| {
                        matches!(outcome, NodeOutcome::Failed | NodeOutcome::Cancelled)
                    })
                };
                if let Some(outcome) = outcome {
                    self.complete(py, key, (outcome, None, (None, None)))?;
                }
            }
        }
        let held: Vec<(String, Completion)> =
            std::mem::take(&mut *self.held.lock().map_err(|_| poisoned())?)
                .into_iter()
                .collect();
        for (key, completion) in held {
            self.complete(py, &key, completion)?;
        }
        let returned = self.returned.load(Ordering::SeqCst);
        let build = self.command().is_build();
        let source_needs_no_run = |key: &str| build && key.starts_with("source:");
        for key in &self.execution.claim.request.selection {
            let outcome = if cancelled {
                NodeOutcome::Cancelled
            } else if returned && source_needs_no_run(key) {
                NodeOutcome::Success
            } else {
                NodeOutcome::Skipped
            };
            self.complete(py, key, (outcome, None, (None, None)))?;
        }
        let outcome = {
            let completed = self.completed.lock().map_err(|_| poisoned())?;
            let outcomes: Vec<NodeOutcome> = completed.values().copied().collect();
            let good = completed.iter().any(|(key, value)| {
                matches!(value, NodeOutcome::Success | NodeOutcome::Warn)
                    && !source_needs_no_run(key)
            });
            let bad = outcomes.iter().any(|value| {
                matches!(
                    value,
                    NodeOutcome::Failed | NodeOutcome::Skipped | NodeOutcome::Cancelled
                )
            });
            if cancelled {
                RunOutcome::Cancelled
            } else if bad && good {
                RunOutcome::Partial
            } else if bad || exit_code != 0 {
                RunOutcome::Failed
            } else if outcomes.contains(&NodeOutcome::Warn) {
                RunOutcome::Warn
            } else {
                RunOutcome::Success
            }
        };
        Ok(outcome)
    }

    /// Terminal publication follows executor teardown and the invocation's bounded Recall drain.
    pub fn finish(&self, py: Python<'_>, exit_code: i32) -> PyResult<RunOutcome> {
        let outcome = self.outcome(py, exit_code)?;
        let unfinished: Vec<String> = self
            .checks
            .lock()
            .map_err(|_| poisoned())?
            .iter()
            .filter(|(_, outcome)| outcome.is_none())
            .map(|(key, _)| key.clone())
            .collect();
        for key in unfinished {
            self.finish_check(
                py,
                &key,
                if outcome == RunOutcome::Cancelled {
                    NodeOutcome::Cancelled
                } else {
                    NodeOutcome::Skipped
                },
            )?;
        }
        let reason: String = self
            .refusal
            .lock()
            .map_err(|_| poisoned())?
            .clone()
            .unwrap_or_else(|| "worker executor completed and closed its connections".into());
        self.worker.bind(py).get().change(
            py,
            "worker-finished",
            RunChange::Finished {
                outcome,
                reason,
                exit_code: Some(i64::from(exit_code)),
            },
        )?;
        Ok(outcome)
    }
}

/// Closing this owner stops and joins heartbeat IO but does not claim warehouse work has stopped.
#[pyclass(frozen, module = "sqlbuild._native")]
#[derive(Debug)]
pub struct NativeOrchestrationWorker {
    pub(crate) owner: Mutex<Option<WorkerOwner>>,
}

impl NativeOrchestrationWorker {
    /// Run `work` on the open owner without holding the GIL; a closed worker is unavailable.
    pub(crate) fn with_owner<T: Send>(
        &self,
        py: Python<'_>,
        work: impl FnOnce(&WorkerOwner) -> PyResult<T> + Send,
    ) -> PyResult<T> {
        py.detach(|| {
            let owner = self.owner.lock().map_err(|_| unavailable())?;
            work(owner.as_ref().ok_or_else(unavailable)?)
        })
    }

    /// SIGTERM may arrive before the periodic heartbeat observes the durable cancellation fact.
    pub fn refresh_cancellation(&self, py: Python<'_>) -> PyResult<bool> {
        self.with_owner(py, |owner| {
            if owner.session.cancel_requested() {
                return Ok(true);
            }
            owner
                .runtime
                .block_on(owner.client.heartbeat())
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Native execution reads the retained request, never caller-provided selection or settings.
    pub fn execution(
        &self,
        py: Python<'_>,
    ) -> PyResult<sqlbuild_orchestration::runs::models::RunExecution> {
        self.with_owner(py, |owner| Ok(owner.session.execution.clone()))
    }

    /// A bounded retry keeps the event identity across an uncertain acknowledgement or busy store.
    pub fn change(&self, py: Python<'_>, key: &str, change: RunChange) -> PyResult<()> {
        self.with_owner(py, |owner| {
            let event_id = Uuid::new_v5(&owner.session.execution.claim.claim_id, key.as_bytes());
            owner.runtime.block_on(async {
                let mut attempts = 0;
                loop {
                    attempts += 1;
                    match owner.client.report(event_id, change.clone()).await {
                        Ok(_) => return Ok(()),
                        Err(error)
                            if attempts < REPORT_ATTEMPTS
                                && match &error {
                                    WorkerError::Transport(_) => true,
                                    WorkerError::Rejected(status) => {
                                        status.as_u16() == STORE_BUSY_STATUS
                                    }
                                    _ => false,
                                } =>
                        {
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                        }
                        Err(error) => return Err(PyRuntimeError::new_err(error.to_string())),
                    }
                }
            })
        })
    }

    /// Verify this retained baseline before the caller applies run settings or opens a warehouse.
    pub fn start(
        snapshot: &NativeOrchestrationSnapshot,
        py: Python<'_>,
        (server_url, bearer): (&str, &str),
        (run_id, claim_id): (String, String),
    ) -> PyResult<NativeOrchestrationWorker> {
        let identity = (
            Uuid::parse_str(&run_id).map_err(|error| PyValueError::new_err(error.to_string()))?,
            Uuid::parse_str(&claim_id).map_err(|error| PyValueError::new_err(error.to_string()))?,
        );
        py.detach(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
            let connected = runtime.block_on(async {
                let client = connect_worker(server_url, bearer, identity)?;
                let session = start_worker(&client, &snapshot.input).await?;
                Ok::<_, sqlbuild_orchestration::worker::errors::WorkerError>((client, session))
            });
            match connected {
                Ok((client, session)) => Ok(NativeOrchestrationWorker {
                    owner: Mutex::new(Some(WorkerOwner {
                        runtime,
                        client,
                        session,
                    })),
                }),
                Err(error) => {
                    runtime.shutdown_timeout(std::time::Duration::from_secs(1));
                    Err(PyRuntimeError::new_err(error.to_string()))
                }
            }
        })
    }
}

#[pymethods]
impl NativeOrchestrationWorker {
    /// The frozen request supplies selection and settings only after baseline verification.
    pub(crate) fn execution_json(&self, py: Python<'_>) -> PyResult<String> {
        self.with_owner(py, |owner| {
            serde_json::to_string(&owner.session.execution)
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    /// Cancellation remains sticky; a closed worker cannot authorize further work.
    pub fn cancel_requested(&self, py: Python<'_>) -> bool {
        py.detach(|| {
            self.owner.lock().map_or(true, |owner| {
                owner
                    .as_ref()
                    .is_none_or(|owner| owner.session.cancel_requested())
            })
        })
    }

    /// Retry an uncertain report with the identical event ID and change, never a new identity.
    pub(crate) fn report(
        &self,
        py: Python<'_>,
        event_id: &str,
        change_json: &str,
    ) -> PyResult<bool> {
        let event_id =
            Uuid::parse_str(event_id).map_err(|error| PyValueError::new_err(error.to_string()))?;
        let change: RunChange = serde_json::from_str(change_json)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        self.with_owner(py, |owner| {
            owner
                .runtime
                .block_on(owner.client.report(event_id, change))
                .map_err(|error| PyRuntimeError::new_err(error.to_string()))
        })
    }

    pub(crate) fn __enter__(slf: PyRef<'_, Self>) -> PyResult<PyRef<'_, Self>> {
        if slf.owner.lock().map_err(|_| unavailable())?.is_none() {
            return Err(unavailable());
        }
        Ok(slf)
    }

    pub(crate) fn __exit__(
        &self,
        py: Python<'_>,
        _kind: &pyo3::Bound<'_, pyo3::PyAny>,
        _value: &pyo3::Bound<'_, pyo3::PyAny>,
        _traceback: &pyo3::Bound<'_, pyo3::PyAny>,
    ) -> PyResult<()> {
        self.close(py)
    }

    /// Join the heartbeat without publishing a terminal outcome or releasing physical claims.
    pub(crate) fn close(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| {
            let mut owner = self.owner.lock().map_err(|_| unavailable())?;
            if let Some(active) = owner.take() {
                active.close();
            }
            Ok(())
        })
    }
}

impl Drop for NativeOrchestrationWorker {
    /// GC can hold the GIL; heartbeat teardown joins native IO and never needs Python.
    fn drop(&mut self) {
        if let Ok(owner) = self.owner.get_mut()
            && let Some(owner) = owner.take()
        {
            owner.close();
        }
    }
}
